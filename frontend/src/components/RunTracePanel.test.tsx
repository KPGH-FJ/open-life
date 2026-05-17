import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import RunTracePanel from "@/components/RunTracePanel";
import type { AgentRunEvent } from "@/types";
import { agentSpecDeniedToolRun, malformedAndUnknownRun } from "@/test/fixtures/agentRunEvents";

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
    payload: { provider: "deepseek", model: "deepseek-chat", step: 1 },
    createdAt: "2026-05-06T10:00:01Z",
  },
  {
    id: "evt-modelfailed",
    runId: "run-001",
    eventType: "model.failed",
    actor: "runtime",
    summary: "Model call failed: LocalOnly blocked cloud",
    payload: { error: "LocalOnly privacy policy requires a local model", recoverable: false },
    createdAt: "2026-05-06T10:00:01.5Z",
  },
  {
    id: "evt-3",
    runId: "run-001",
    eventType: "tool.call_blocked",
    actor: "runtime",
    summary: "email.read blocked",
    payload: { tool: "email.read", reason: "declarative-only" },
    createdAt: "2026-05-06T10:00:02Z",
  },
  {
    id: "evt-4",
    runId: "run-001",
    eventType: "plan.created",
    actor: "agent",
    summary: "Plan created: Analyze project",
    payload: { plan_id: "plan-1", goal: "Analyze project structure", risk_level: "low" },
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
  {
    id: "evt-redacted",
    runId: "run-001",
    eventType: "model.call_completed",
    actor: "agent",
    summary: "Model returned with data redaction",
    payload: { reply_len: 512 },
    redaction: {
      redacted: true,
      reason: "PII detected in model output",
      fieldsRemoved: ["email", "phone"],
    },
    createdAt: "2026-05-06T10:00:04.5Z",
  },
  {
    id: "evt-truncated",
    runId: "run-001",
    eventType: "tool.call_completed",
    actor: { tool: "file.read" },
    summary: "file.read completed with truncated output",
    payload: { tool: "file.read", output_truncated: true, truncated: true, content_length: 50000 },
    createdAt: "2026-05-06T10:00:05.5Z",
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
    expect(screen.getByText(/13 events/)).toBeDefined();
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
        summary: "Future event type",
        payload: {},
        createdAt: "2026-05-06T10:00:01Z",
      },
    ];
    render(
      <RunTracePanel events={unknownEvents} runId="run-001" show={true} onToggle={() => {}} />
    );
    expect(screen.getByText("Future event type")).toBeDefined();
  });

  it("toggles expansion when clicked", async () => {
    let show = false;
    const onToggle = () => {
      show = !show;
    };
    const { rerender } = render(
      <RunTracePanel events={mockEvents} runId="run-001" show={show} onToggle={onToggle} />
    );

    expect(screen.queryByText("Agent run created")).toBeNull();

    const button = screen.getByTestId("run-trace-toggle");
    await userEvent.click(button);

    rerender(<RunTracePanel events={mockEvents} runId="run-001" show={true} onToggle={onToggle} />);

    expect(screen.getByText("Agent run created")).toBeDefined();
  });

  it("renders compaction.created event", () => {
    render(<RunTracePanel events={mockEvents} runId="run-001" show={true} onToggle={() => {}} />);
    expect(screen.getByText("compaction.created")).toBeDefined();
    expect(screen.getByText(/Context compacted:/)).toBeDefined();
  });

  it("renders shell.blocked event", () => {
    const shellEvents: AgentRunEvent[] = [
      {
        id: "evt-shell-1",
        runId: "run-001",
        eventType: "shell.blocked",
        actor: "runtime",
        summary: "shell.run blocked: shell execution is disabled",
        payload: { tool: "shell.run" },
        createdAt: "2026-05-07T10:00:03Z",
      },
    ];
    render(<RunTracePanel events={shellEvents} runId="run-001" show={true} onToggle={() => {}} />);
    expect(screen.getByText("shell.blocked")).toBeDefined();
    expect(screen.getByText("shell.run blocked: shell execution is disabled")).toBeDefined();
  });

  it("renders shell.completed event as governed tool event", () => {
    const shellEvents: AgentRunEvent[] = [
      {
        id: "evt-shell-2",
        runId: "run-001",
        eventType: "shell.completed",
        actor: { tool: "shell.run" },
        summary: "shell.run completed: ls /tmp",
        payload: { command: "ls", args: ["/tmp"], exit_code: 0, output_truncated: true },
        createdAt: "2026-05-07T10:00:05Z",
      },
    ];
    render(<RunTracePanel events={shellEvents} runId="run-001" show={true} onToggle={() => {}} />);
    expect(screen.getByText("shell.completed")).toBeDefined();
  });

  it("shows redaction metadata badge on events with redaction", () => {
    render(<RunTracePanel events={mockEvents} runId="run-001" show={true} onToggle={() => {}} />);
    const redactionBadges = screen.getAllByText("脱敏");
    expect(redactionBadges.length).toBeGreaterThan(0);
  });

  it("shows truncated output marker on events with truncated payload", () => {
    render(<RunTracePanel events={mockEvents} runId="run-001" show={true} onToggle={() => {}} />);
    const truncatedBadges = screen.getAllByText("已截断");
    expect(truncatedBadges.length).toBeGreaterThan(0);
  });

  it("expands event detail panel on click showing payload metadata", async () => {
    render(<RunTracePanel events={mockEvents} runId="run-001" show={true} onToggle={() => {}} />);

    const modelStartedBtn = screen.getByTestId("event-row-evt-2");
    await userEvent.click(modelStartedBtn);

    expect(screen.getByText("事件载荷")).toBeDefined();
  });

  it("shows redaction detail in expanded panel", async () => {
    render(<RunTracePanel events={mockEvents} runId="run-001" show={true} onToggle={() => {}} />);

    const redactedBtn = screen.getByTestId("event-row-evt-redacted");
    await userEvent.click(redactedBtn);

    expect(screen.getByText("脱敏信息")).toBeDefined();
    expect(screen.getByText(/PII detected in model output/)).toBeDefined();
    expect(screen.getByText(/email, phone/)).toBeDefined();
  });

  it("shows truncated marker detail in expanded panel", async () => {
    render(<RunTracePanel events={mockEvents} runId="run-001" show={true} onToggle={() => {}} />);

    const truncatedBtn = screen.getByTestId("event-row-evt-truncated");
    await userEvent.click(truncatedBtn);

    expect(screen.getByText(/输出已截断/)).toBeDefined();
  });

  it("applies visual border classes for event grouping", () => {
    render(<RunTracePanel events={mockEvents} runId="run-001" show={true} onToggle={() => {}} />);
    // Model events use purple left border
    const modelEvent = screen.getByTestId("event-row-evt-2");
    expect(modelEvent.parentElement?.className).toContain("border-l-purple-400");
    // Plan events use cyan left border
    const planEvent = screen.getByTestId("event-row-evt-4");
    expect(planEvent.parentElement?.className).toContain("border-l-cyan-400");
  });

  it("does not crash when payload has non-serializable values", () => {
    const trickyEvents: AgentRunEvent[] = [
      {
        id: "evt-tricky",
        runId: "run-001",
        eventType: "observation.created",
        actor: "agent",
        summary: "Tricky observation",
        payload: { circular: "manually handled" },
        createdAt: "2026-05-06T10:00:01Z",
      },
    ];
    render(<RunTracePanel events={trickyEvents} runId="run-001" show={true} onToggle={() => {}} />);
    expect(screen.getByText("Tricky observation")).toBeDefined();
  });

  it("hides sensitive payload fields when event is redacted", async () => {
    const redactedEmail: AgentRunEvent[] = [
      {
        id: "evt-redact-email",
        runId: "run-001",
        eventType: "model.call_completed",
        actor: "agent",
        summary: "Model reply with email",
        payload: { reply: "ok", email: "alice@example.com", phone: "+8613900000000" },
        redaction: { redacted: true, reason: "PII detected", fieldsRemoved: ["email"] },
        createdAt: "2026-05-06T10:00:00Z",
      },
    ];
    render(
      <RunTracePanel events={redactedEmail} runId="run-001" show={true} onToggle={() => {}} />
    );

    await userEvent.click(screen.getByTestId("event-row-evt-redact-email"));

    // Sensitive values must NOT appear in DOM
    const panelHtml = document.querySelector(".bg-slate-950\\/50")?.innerHTML ?? "";
    expect(panelHtml).not.toContain("alice@example.com");
    expect(panelHtml).not.toContain("+8613900000000");

    // [已隐藏] placeholder appears for redacted/sensitive fields
    const hiddenTexts = screen.getAllByText(/已隐藏/);
    expect(hiddenTexts.length).toBeGreaterThan(0);
  });

  it("non-redacted payloads still display safe summary values", async () => {
    const normalEvent: AgentRunEvent[] = [
      {
        id: "evt-normal",
        runId: "run-001",
        eventType: "model.call_completed",
        actor: "agent",
        summary: "Normal model reply",
        payload: { reply_len: 256, provider: "deepseek" },
        createdAt: "2026-05-06T10:00:00Z",
      },
    ];
    render(<RunTracePanel events={normalEvent} runId="run-001" show={true} onToggle={() => {}} />);

    await userEvent.click(screen.getByTestId("event-row-evt-normal"));

    expect(screen.getByText(/reply_len/)).toBeDefined();
    expect(screen.getByText(/deepseek/)).toBeDefined();
    expect(screen.queryByText("[已隐藏]")).toBeNull();
  });

  it("hides sensitive key values even when not in fieldsRemoved list", async () => {
    const tokenLeak: AgentRunEvent[] = [
      {
        id: "evt-token",
        runId: "run-001",
        eventType: "model.call_completed",
        actor: "agent",
        summary: "Model reply with hidden token",
        payload: { api_key: "sk-secret-key-123", model: "deepseek-chat" },
        redaction: { redacted: true, reason: "credentials stripped", fieldsRemoved: [] },
        createdAt: "2026-05-06T10:00:00Z",
      },
    ];
    render(<RunTracePanel events={tokenLeak} runId="run-001" show={true} onToggle={() => {}} />);

    await userEvent.click(screen.getByTestId("event-row-evt-token"));

    const panelHtml = document.querySelector(".bg-slate-950\\/50")?.innerHTML ?? "";
    expect(panelHtml).not.toContain("sk-secret-key-123");
    const hiddenTexts = screen.getAllByText(/已隐藏/);
    expect(hiddenTexts.length).toBeGreaterThan(0);
    // Non-sensitive keys still shown
    expect(panelHtml).toContain("deepseek-chat");
  });

  it("hides nested email/token in redacted events", async () => {
    const nestedEvent: AgentRunEvent[] = [
      {
        id: "evt-nested",
        runId: "run-001",
        eventType: "model.call_completed",
        actor: "agent",
        summary: "Nested sensitive data",
        payload: {
          result: {
            email: "alice@example.com",
            token: "sk-secret-nested",
            safe_field: "public info",
          },
        },
        redaction: { redacted: true, reason: "PII in result", fieldsRemoved: [] },
        createdAt: "2026-05-06T10:00:00Z",
      },
    ];
    render(<RunTracePanel events={nestedEvent} runId="run-001" show={true} onToggle={() => {}} />);
    await userEvent.click(screen.getByTestId("event-row-evt-nested"));

    const panelHtml = document.querySelector(".bg-slate-950\\/50")?.innerHTML ?? "";
    expect(panelHtml).not.toContain("alice@example.com");
    expect(panelHtml).not.toContain("sk-secret-nested");
    expect(panelHtml).toContain("public info");
  });

  it("respects dot-path fieldsRemoved for nested fields", async () => {
    const dotPathEvent: AgentRunEvent[] = [
      {
        id: "evt-dotpath",
        runId: "run-001",
        eventType: "model.call_completed",
        actor: "agent",
        summary: "Dot path redaction",
        payload: {
          data: { secret_field: "should-be-hidden", visible_field: "ok" },
        },
        redaction: {
          redacted: true,
          reason: "path redaction",
          fieldsRemoved: ["data.secret_field"],
        },
        createdAt: "2026-05-06T10:00:00Z",
      },
    ];
    render(<RunTracePanel events={dotPathEvent} runId="run-001" show={true} onToggle={() => {}} />);
    await userEvent.click(screen.getByTestId("event-row-evt-dotpath"));

    const panelHtml = document.querySelector(".bg-slate-950\\/50")?.innerHTML ?? "";
    expect(panelHtml).not.toContain("should-be-hidden");
    expect(panelHtml).toContain("ok");
    expect(screen.getAllByText(/已隐藏/).length).toBeGreaterThan(0);
  });

  it("fieldsRemoved pointing to subtree hides entire subtree", async () => {
    const subtreeEvent: AgentRunEvent[] = [
      {
        id: "evt-subtree",
        runId: "run-001",
        eventType: "model.call_completed",
        actor: "agent",
        summary: "Subtree redaction",
        payload: {
          life_model: { name: "John", secret_token: "xyz", visible: "yes" },
          other: "safe",
        },
        redaction: {
          redacted: true,
          reason: "life_model subtree",
          fieldsRemoved: ["life_model"],
        },
        createdAt: "2026-05-06T10:00:00Z",
      },
    ];
    render(<RunTracePanel events={subtreeEvent} runId="run-001" show={true} onToggle={() => {}} />);
    await userEvent.click(screen.getByTestId("event-row-evt-subtree"));

    const panelHtml = document.querySelector(".bg-slate-950\\/50")?.innerHTML ?? "";
    // Entire life_model subtree hidden
    expect(panelHtml).not.toContain("John");
    expect(panelHtml).not.toContain("secret_token");
    expect(panelHtml).not.toContain("yes");
    // Non-redacted field visible
    expect(panelHtml).toContain("safe");
  });

  it("non-redacted nested payloads display safe summaries", async () => {
    const normalNested: AgentRunEvent[] = [
      {
        id: "evt-normal-nested",
        runId: "run-001",
        eventType: "model.call_completed",
        actor: "agent",
        summary: "Normal nested data",
        payload: {
          stats: { latency_ms: 123, tokens: 50 },
          provider: "deepseek",
        },
        createdAt: "2026-05-06T10:00:00Z",
      },
    ];
    render(<RunTracePanel events={normalNested} runId="run-001" show={true} onToggle={() => {}} />);
    await userEvent.click(screen.getByTestId("event-row-evt-normal-nested"));

    const panelHtml = document.querySelector(".bg-slate-950\\/50")?.innerHTML ?? "";
    expect(panelHtml).toContain("latency_ms");
    expect(panelHtml).toContain("123");
    expect(panelHtml).toContain("deepseek");
    expect(panelHtml).not.toContain("[已隐藏]");
  });

  // ── Batch 5: Typed Execution Contract tests ───────────────────────────

  it("renders tool blocked typed payload with block_reason and agent_spec_id", async () => {
    const typedBlocked: AgentRunEvent[] = [
      {
        id: "evt-typed-blocked",
        runId: "run-001",
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
        createdAt: "2026-05-15T10:00:00Z",
      },
    ];
    render(<RunTracePanel events={typedBlocked} runId="run-001" show={true} onToggle={() => {}} />);
    await userEvent.click(screen.getByTestId("event-row-evt-typed-blocked"));
    expect(screen.getByText("阻断详情 (Typed Contract)")).toBeDefined();
    expect(screen.getAllByText("AgentSpec 拒绝").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("main.default").length).toBeGreaterThanOrEqual(1);
    // Tool name appears in both explanation and detail blocks
    expect(screen.getAllByText("web.search").length).toBeGreaterThanOrEqual(1);
  });

  it("renders replay failed typed payload with block_reason", async () => {
    const replayFailed: AgentRunEvent[] = [
      {
        id: "evt-replay-fail",
        runId: "run-001",
        eventType: "replay.failed",
        actor: "runtime",
        summary: "Replay failed: missing spec",
        payload: {
          status: "failed",
          run_id: "run-001",
          action_id: "action-replay-2",
          replay_of_action_id: "action-original-2",
          human_message: "Replay failed: missing action spec",
          block_reason: "replay_spec_missing",
          failure_kind: null,
          tool_name: "remote_tool",
          source: "mcp:my-server",
          agent_spec_id: "main.default",
        },
        createdAt: "2026-05-15T10:01:00Z",
      },
    ];
    render(<RunTracePanel events={replayFailed} runId="run-001" show={true} onToggle={() => {}} />);
    await userEvent.click(screen.getByTestId("event-row-evt-replay-fail"));
    expect(screen.getByText("重放失败 (Typed Contract)")).toBeDefined();
    expect(screen.getAllByText("缺少重放规格").length).toBeGreaterThanOrEqual(1);
    // Tool name "remote_tool" appears in explanation debugFacts and detail block
    expect(screen.getAllByText("remote_tool").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("mcp:my-server").length).toBeGreaterThanOrEqual(1);
  });

  it("renders mcp target block wrapper and target fields", async () => {
    const mcpTargetBlock: AgentRunEvent[] = [
      {
        id: "evt-mcp-target",
        runId: "run-001",
        eventType: "tool.call_blocked",
        actor: "runtime",
        summary: "mcp.call_tool target blocked",
        payload: {
          status: "blocked",
          tool_name: "mcp.call_tool",
          source: "builtin",
          target_tool_name: "remote_search",
          target_source: "mcp:my-server",
          wrapper_tool_name: "mcp.call_tool",
          block_reason: "tool_permission_denied",
          proposal_reason: null,
          failure_kind: null,
          agent_spec_id: "main.default",
        },
        createdAt: "2026-05-15T10:00:00Z",
      },
    ];
    render(
      <RunTracePanel events={mcpTargetBlock} runId="run-001" show={true} onToggle={() => {}} />
    );
    await userEvent.click(screen.getByTestId("event-row-evt-mcp-target"));
    expect(screen.getByText("MCP 包装:")).toBeDefined();
    expect(screen.getAllByText("remote_search").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("mcp:my-server").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("工具权限拒绝").length).toBeGreaterThanOrEqual(1);
  });

  it("renders network policy ask with proposal_id", async () => {
    const networkAsk: AgentRunEvent[] = [
      {
        id: "evt-net-ask",
        runId: "run-001",
        eventType: "tool.call_blocked",
        actor: "runtime",
        summary: "web.search needs confirmation",
        payload: {
          status: "needs_confirmation",
          tool_name: "web.search",
          source: "builtin",
          block_reason: null,
          proposal_reason: "network_policy_ask",
          proposal_id: "proposal-net-1",
          failure_kind: null,
          agent_spec_id: "main.default",
        },
        createdAt: "2026-05-15T10:00:00Z",
      },
    ];
    render(<RunTracePanel events={networkAsk} runId="run-001" show={true} onToggle={() => {}} />);
    await userEvent.click(screen.getByTestId("event-row-evt-net-ask"));
    expect(screen.getAllByText("网络策略询问").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("proposal-net-1").length).toBeGreaterThanOrEqual(1);
  });

  it("does not infer reason from message text when typed fields are absent", async () => {
    // Old-style event with payload missing required typed fields
    const legacyEvent: AgentRunEvent[] = [
      {
        id: "evt-legacy",
        runId: "run-001",
        eventType: "tool.call_blocked",
        actor: "runtime",
        summary: "email.read blocked: declarative-only stub",
        payload: { tool: "email.read", reason: "declarative-only" },
        createdAt: "2026-05-06T10:00:00Z",
      },
    ];
    render(<RunTracePanel events={legacyEvent} runId="run-001" show={true} onToggle={() => {}} />);
    await userEvent.click(screen.getByTestId("event-row-evt-legacy"));
    // Payload fails structural validation (no status, no tool_name, no source) →
    // parsed as unknown → typed contract section does NOT render
    expect(screen.queryByText("阻断详情 (Typed Contract)")).toBeNull();
    // Should NOT have typed reason badges since none present
    expect(screen.queryByText("AgentSpec 拒绝")).toBeNull();
    expect(screen.queryByText("工具权限拒绝")).toBeNull();
  });

  it("renders replay started and completed typed payloads", async () => {
    const replayEvents: AgentRunEvent[] = [
      {
        id: "evt-replay-start",
        runId: "run-001",
        eventType: "replay.started",
        actor: "runtime",
        summary: "Replay started",
        payload: {
          status: "started",
          run_id: "run-001",
          action_id: "action-replay-1",
          replay_of_action_id: "action-orig-1",
          agent_spec_id: "main.default",
          tool_name: "web.search",
          source: "builtin",
        },
        createdAt: "2026-05-15T10:01:00Z",
      },
      {
        id: "evt-replay-done",
        runId: "run-001",
        eventType: "replay.completed",
        actor: "runtime",
        summary: "Replay completed",
        payload: {
          status: "completed",
          run_id: "run-001",
          action_id: "action-replay-1",
          replay_of_action_id: "action-orig-1",
          agent_spec_id: "main.default",
          tool_name: "web.search",
          source: "builtin",
          block_reason: null,
          proposal_reason: null,
          failure_kind: null,
        },
        createdAt: "2026-05-15T10:01:01Z",
      },
    ];
    render(<RunTracePanel events={replayEvents} runId="run-001" show={true} onToggle={() => {}} />);
    await userEvent.click(screen.getByTestId("event-row-evt-replay-start"));
    expect(screen.getByText("重放开始 (Typed Contract)")).toBeDefined();
    await userEvent.click(screen.getByTestId("event-row-evt-replay-done"));
    expect(screen.getByText("重放完成 (Typed Contract)")).toBeDefined();
    expect(screen.getByText("成功")).toBeDefined();
  });

  it("renders replay completed blocked with typed reason", async () => {
    const replayBlocked: AgentRunEvent[] = [
      {
        id: "evt-replay-blocked",
        runId: "run-001",
        eventType: "replay.completed",
        actor: "runtime",
        summary: "Replay completed but blocked",
        payload: {
          status: "blocked",
          run_id: "run-001",
          action_id: "action-replay-3",
          replay_of_action_id: "action-orig-3",
          agent_spec_id: "main.default",
          tool_name: "web.search",
          source: "builtin",
          block_reason: "replay_spec_missing",
          proposal_reason: null,
          failure_kind: null,
        },
        createdAt: "2026-05-15T10:01:01Z",
      },
    ];
    render(
      <RunTracePanel events={replayBlocked} runId="run-001" show={true} onToggle={() => {}} />
    );
    await userEvent.click(screen.getByTestId("event-row-evt-replay-blocked"));
    expect(screen.getByText("重放完成 (Typed Contract)")).toBeDefined();
    expect(screen.getByText("缺少重放规格")).toBeDefined();
    expect(screen.getByText("已阻断")).toBeDefined();
  });

  // ── Batch 6: Explainability tests ────────────────────────────────────

  it("expanded typed event shows user-facing explanation before raw payload", async () => {
    const typedBlocked: AgentRunEvent[] = [
      {
        id: "evt-explain-blocked",
        runId: "run-001",
        eventType: "tool.call_blocked",
        actor: "runtime",
        summary: "web.search blocked by AgentSpec",
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
    render(<RunTracePanel events={typedBlocked} runId="run-001" show={true} onToggle={() => {}} />);
    await userEvent.click(screen.getByTestId("event-row-evt-explain-blocked"));

    // Event explanation block should render (user-facing)
    expect(screen.getByTestId("event-explanation-evt-explain-blocked")).toBeDefined();
    // User-facing explanation describes what happened
    expect(screen.getAllByText(/工具.*被.*阻断/).length).toBeGreaterThanOrEqual(1);
    // Typed reason appears in explanation
    expect(screen.getAllByText(/AgentSpec 拒绝/).length).toBeGreaterThanOrEqual(1);
    // Raw payload still available
    expect(screen.getByText("事件载荷")).toBeDefined();
  });

  it("unknown / malformed event shows fallback explanation and does not crash", async () => {
    const unknownEvent: AgentRunEvent[] = [
      {
        id: "evt-explain-unknown",
        runId: "run-001",
        eventType: "run.created",
        actor: "runtime",
        summary: "Simple run created",
        payload: { session_id: "sess-1" },
        createdAt: "2026-05-16T10:00:00Z",
      },
    ];
    render(<RunTracePanel events={unknownEvent} runId="run-001" show={true} onToggle={() => {}} />);
    await userEvent.click(screen.getByTestId("event-row-evt-explain-unknown"));

    // Fallback explanation should render
    expect(screen.getByTestId("event-explanation-evt-explain-unknown")).toBeDefined();
    // Fallback message is user-friendly
    expect(screen.getByText("这是一个未识别的运行事件")).toBeDefined();
    // Raw payload still available
    expect(screen.getByText("事件载荷")).toBeDefined();
  });

  it("malformed tool.call_blocked with no typed fields shows fallback, no crash", async () => {
    const malformed: AgentRunEvent[] = [
      {
        id: "evt-explain-malformed",
        runId: "run-001",
        eventType: "tool.call_blocked",
        actor: "runtime",
        summary: "broken block event",
        payload: { tool: "email.read", reason: "declarative-only" },
        createdAt: "2026-05-16T10:00:00Z",
      },
    ];
    render(<RunTracePanel events={malformed} runId="run-001" show={true} onToggle={() => {}} />);
    await userEvent.click(screen.getByTestId("event-row-evt-explain-malformed"));

    // Fallback explanation should render (payload lacks typed fields → unknown)
    expect(screen.getByTestId("event-explanation-evt-explain-malformed")).toBeDefined();
    expect(screen.getByText("这是一个未识别的运行事件")).toBeDefined();
  });

  it("event explanation does not infer reason from summary text", async () => {
    // Summary text says "network_policy_denied" but payload has no typed fields
    const summaryEvent: AgentRunEvent[] = [
      {
        id: "evt-explain-summary",
        runId: "run-001",
        eventType: "replay.failed",
        actor: "runtime",
        summary: "Replay failed: network_policy_denied in summary (IGNORE)",
        payload: {
          status: "failed",
          run_id: "run-001",
          action_id: "a1",
          replay_of_action_id: "orig-1",
          human_message: "Error: network_policy_denied (noise in human_message)",
          // NO block_reason, NO failure_kind → fails structural validation
        },
        createdAt: "2026-05-16T10:00:00Z",
      },
    ];
    render(<RunTracePanel events={summaryEvent} runId="run-001" show={true} onToggle={() => {}} />);
    await userEvent.click(screen.getByTestId("event-row-evt-explain-summary"));

    // Payload fails structural validation → fallback explanation
    expect(screen.getByTestId("event-explanation-evt-explain-summary")).toBeDefined();
    expect(screen.getByText("这是一个未识别的运行事件")).toBeDefined();
    // Must NOT contain "网络策略拒绝" from summary text
    expect(screen.queryByText("网络策略拒绝")).toBeNull();
  });

  it("raw payload remains available in expanded debug section alongside explanation", async () => {
    const replayFailed: AgentRunEvent[] = [
      {
        id: "evt-explain-replayfail",
        runId: "run-001",
        eventType: "replay.failed",
        actor: "runtime",
        summary: "Replay failed",
        payload: {
          status: "failed",
          run_id: "run-001",
          action_id: "a1",
          replay_of_action_id: "orig-1",
          block_reason: "replay_spec_missing",
          human_message: "details",
        },
        createdAt: "2026-05-16T10:00:00Z",
      },
    ];
    render(<RunTracePanel events={replayFailed} runId="run-001" show={true} onToggle={() => {}} />);
    await userEvent.click(screen.getByTestId("event-row-evt-explain-replayfail"));

    // Explanation is present
    expect(screen.getByTestId("event-explanation-evt-explain-replayfail")).toBeDefined();
    // Raw payload section is also present
    expect(screen.getByText("事件载荷")).toBeDefined();
    // Typed detail block is also present
    expect(screen.getByText("重放失败 (Typed Contract)")).toBeDefined();
  });

  // ── Fixture-based explainability end-to-end tests ──────────────────

  it("fixture: agentSpecDeniedToolRun — events render without crash, typed detail visible", async () => {
    render(
      <RunTracePanel
        events={agentSpecDeniedToolRun}
        runId="run-fixture"
        show={true}
        onToggle={() => {}}
      />
    );
    // Expand the blocked event
    const evtId = agentSpecDeniedToolRun.find(e => e.eventType === "tool.call_blocked")!.id;
    await userEvent.click(screen.getByTestId(`event-row-${evtId}`));
    // User-facing explanation before raw payload
    expect(screen.getByTestId(`event-explanation-${evtId}`)).toBeDefined();
    // Typed contract detail visible
    expect(screen.getByText("阻断详情 (Typed Contract)")).toBeDefined();
    expect(screen.getByText("AgentSpec 拒绝")).toBeDefined();
    expect(screen.getAllByText("main.strict").length).toBeGreaterThanOrEqual(1);
  });

  it("fixture: malformedAndUnknownRun — UI does not crash, malformed events show fallback", async () => {
    render(
      <RunTracePanel
        events={malformedAndUnknownRun}
        runId="run-fixture"
        show={true}
        onToggle={() => {}}
      />
    );
    // All events render without crash
    for (const evt of malformedAndUnknownRun) {
      expect(screen.getByTestId(`event-row-${evt.id}`)).toBeDefined();
    }
    // Expand the malformed tool.call_blocked
    const blockedEvt = malformedAndUnknownRun.find(e => e.eventType === "tool.call_blocked")!;
    await userEvent.click(screen.getByTestId(`event-row-${blockedEvt.id}`));
    // Fallback explanation (no typed reason parsed for invalid reason value)
    expect(screen.getByTestId(`event-explanation-${blockedEvt.id}`)).toBeDefined();
    // Must NOT infer reason from summary text
    expect(screen.queryByText("AgentSpec 拒绝")).toBeNull();
  });
});
