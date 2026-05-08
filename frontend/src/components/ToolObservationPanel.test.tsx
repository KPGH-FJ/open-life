import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import ToolObservationPanel from "@/components/ToolObservationPanel";
import type { AgentRun } from "@/tauri";

function makeRun(overrides: Partial<AgentRun> = {}): AgentRun {
  return {
    id: "run-test",
    taskId: "task-1",
    sessionId: "sess-1",
    status: "completed",
    kind: "conversation",
    startedAt: "2026-05-08T10:00:00Z",
    finishedAt: "2026-05-08T10:01:00Z",
    actions: [],
    observations: [],
    generatedProposals: [],
    ...overrides,
  };
}

describe("ToolObservationPanel", () => {
  it("shows empty state when no tools or observations", () => {
    render(
      <MemoryRouter>
        <ToolObservationPanel run={makeRun()} />
      </MemoryRouter>
    );
    expect(screen.getByText("此运行中没有工具调用或观察记录。")).toBeDefined();
  });

  it("renders successful tool observation", () => {
    const run = makeRun({
      actions: [
        {
          id: "action-1",
          actionType: "mcp_tool_call",
          status: "succeeded",
          input: {},
          output: { result: "ok" },
          timestamp: "2026-05-08T10:00:05Z",
          toolScope: {
            toolId: "life_model.read",
            toolName: "life_model.read",
            source: "builtin",
            riskLevel: "low",
            capabilities: ["read"],
            actionType: "read",
          },
        },
      ],
    });
    render(
      <MemoryRouter>
        <ToolObservationPanel run={run} />
      </MemoryRouter>
    );
    expect(screen.getByText("life_model.read")).toBeDefined();
    expect(screen.getByText("成功")).toBeDefined();
  });

  it("renders blocked tool observation with reason", async () => {
    const run = makeRun({
      actions: [
        {
          id: "action-blocked",
          actionType: "tool_call",
          status: "blocked",
          input: {},
          timestamp: "2026-05-08T10:00:05Z",
          toolScope: {
            toolId: "file.write",
            toolName: "file.write",
            source: "builtin",
            riskLevel: "high",
            capabilities: [],
            actionType: "write",
          },
        },
      ],
    });
    render(
      <MemoryRouter>
        <ToolObservationPanel run={run} />
      </MemoryRouter>
    );
    expect(screen.getByText("file.write")).toBeDefined();
    expect(screen.getByText("已阻断")).toBeDefined();

    // Expand to see reason
    await userEvent.click(screen.getByText("file.write"));
    expect(screen.getByText("阻断原因")).toBeDefined();
    expect(screen.getByText("该工具调用被权限策略或沙盒规则阻断。")).toBeDefined();
  });

  it("shows high-risk tool scope badge", () => {
    const run = makeRun({
      actions: [
        {
          id: "action-high",
          actionType: "tool_call",
          status: "succeeded",
          input: {},
          output: {},
          timestamp: "2026-05-08T10:00:05Z",
          toolScope: {
            toolId: "file.write_proposal",
            toolName: "file.write_proposal",
            source: "builtin",
            riskLevel: "high",
            capabilities: ["write"],
            actionType: "write",
          },
        },
      ],
    });
    render(
      <MemoryRouter>
        <ToolObservationPanel run={run} />
      </MemoryRouter>
    );
    expect(screen.getByText("file.write_proposal")).toBeDefined();
    expect(screen.getByText("高风险")).toBeDefined();
  });

  it("shows declarative-only label for declarative tools", () => {
    const run = makeRun({
      actions: [
        {
          id: "action-decl",
          actionType: "mcp_tool_call",
          status: "blocked",
          input: {},
          timestamp: "2026-05-08T10:00:05Z",
          toolScope: {
            toolId: "email.read",
            toolName: "email.read",
            source: "builtin",
            riskLevel: "low",
            capabilities: ["declarative_only"],
            actionType: "read",
          },
        },
      ],
    });
    render(
      <MemoryRouter>
        <ToolObservationPanel run={run} />
      </MemoryRouter>
    );
    expect(screen.getByText("email.read")).toBeDefined();
    expect(screen.getByText("声明-only")).toBeDefined();
  });

  it("collapses large outputs and shows bounded preview marker", async () => {
    const tailSentinel = "TAIL_SENTINEL_MARKER";
    const longOutput = "x".repeat(900) + tailSentinel;
    const run = makeRun({
      actions: [
        {
          id: "action-long",
          actionType: "tool_call",
          status: "succeeded",
          input: {},
          output: longOutput,
          timestamp: "2026-05-08T10:00:05Z",
          toolScope: {
            toolId: "file.read",
            toolName: "file.read",
            source: "builtin",
            riskLevel: "low",
            capabilities: ["read"],
            actionType: "read",
          },
        },
      ],
    });
    render(
      <MemoryRouter>
        <ToolObservationPanel run={run} />
      </MemoryRouter>
    );
    await userEvent.click(screen.getByText("file.read"));

    // Tail sentinel must NOT appear (output is bounded)
    const panelHtml = document.body.innerHTML;
    expect(panelHtml).not.toContain(tailSentinel);
    // Truncation marker with estimated length
    expect(screen.getByText(/估算.*\d+.*字符/)).toBeDefined();
  });

  it("does not expose tail of super-long string in DOM", async () => {
    const tailSentinel = "SHOULD_NOT_APPEAR_ANYWHERE";
    const superLong = "a".repeat(2000) + tailSentinel;
    const run = makeRun({
      actions: [
        {
          id: "action-superlong",
          actionType: "tool_call",
          status: "succeeded",
          input: {},
          output: superLong,
          timestamp: "2026-05-08T10:00:05Z",
          toolScope: {
            toolId: "file.read",
            toolName: "file.read",
            source: "builtin",
            riskLevel: "low",
            capabilities: ["read"],
            actionType: "read",
          },
        },
      ],
    });
    render(
      <MemoryRouter>
        <ToolObservationPanel run={run} />
      </MemoryRouter>
    );
    await userEvent.click(screen.getByText("file.read"));
    expect(document.body.innerHTML).not.toContain(tailSentinel);
    expect(screen.getByText(/已截断/)).toBeDefined();
  });

  it("bounded preview for large object output", async () => {
    const largeObj: Record<string, string> = {};
    for (let i = 0; i < 200; i++) {
      largeObj[`key_${i}`] = `value_${i}_with_some_padding`;
    }
    const tailKey = "key_199";
    const run = makeRun({
      actions: [
        {
          id: "action-largeobj",
          actionType: "tool_call",
          status: "succeeded",
          input: {},
          output: largeObj,
          timestamp: "2026-05-08T10:00:05Z",
          toolScope: {
            toolId: "state.read",
            toolName: "state.read",
            source: "builtin",
            riskLevel: "low",
            capabilities: ["read"],
            actionType: "read",
          },
        },
      ],
    });
    render(
      <MemoryRouter>
        <ToolObservationPanel run={run} />
      </MemoryRouter>
    );
    await userEvent.click(screen.getByText("state.read"));

    // Deep tail key should not appear (large object is bounded)
    expect(document.body.innerHTML).not.toContain(tailKey);
    expect(screen.getByText(/已截断/)).toBeDefined();
  });

  it("bounded preview for observation content", async () => {
    const tailSentinel = "OBS_TAIL_SECRET";
    const longContent = "y".repeat(700) + tailSentinel;
    const run = makeRun({
      observations: [
        {
          id: "obs-long",
          content: longContent,
          source: "web.fetch",
          timestamp: "2026-05-08T10:00:10Z",
        },
      ],
    });
    render(
      <MemoryRouter>
        <ToolObservationPanel run={run} />
      </MemoryRouter>
    );
    await userEvent.click(screen.getByText("观察记录 (1)"));
    expect(document.body.innerHTML).not.toContain(tailSentinel);
    expect(screen.getByText(/估算长度.*\d+.*字符/)).toBeDefined();
  });

  it("bounded preview for observation structuredResult", async () => {
    const largeStruct: Record<string, string> = {};
    for (let i = 0; i < 100; i++) {
      largeStruct[`item_${i}`] = `data_${i}_padding_padding`;
    }
    const tailKey = "item_99";
    const run = makeRun({
      observations: [
        {
          id: "obs-struct",
          content: "result received",
          source: "analysis",
          structuredResult: largeStruct,
          timestamp: "2026-05-08T10:00:10Z",
        },
      ],
    });
    render(
      <MemoryRouter>
        <ToolObservationPanel run={run} />
      </MemoryRouter>
    );
    await userEvent.click(screen.getByText("观察记录 (1)"));
    expect(document.body.innerHTML).not.toContain(tailKey);
  });

  it("shows observation records with content", async () => {
    const run = makeRun({
      actions: [],
      observations: [
        {
          id: "obs-1",
          content: "文件 /tmp/test.txt 的内容: hello world",
          source: "file.read",
          structuredResult: { size: 11 },
          timestamp: "2026-05-08T10:00:10Z",
        },
      ],
    });
    render(
      <MemoryRouter>
        <ToolObservationPanel run={run} />
      </MemoryRouter>
    );
    expect(screen.getByText("观察记录 (1)")).toBeDefined();
    await userEvent.click(screen.getByText("观察记录 (1)"));
    expect(screen.getByText("文件 /tmp/test.txt 的内容: hello world")).toBeDefined();
    expect(screen.getByText("来源: file.read")).toBeDefined();
  });

  it("shows needs_confirmation status with link to review", async () => {
    const run = makeRun({
      actions: [
        {
          id: "action-confirm",
          actionType: "mcp_tool_call",
          status: "needs_confirmation",
          permissionDecision: "ask_every_time",
          input: {},
          timestamp: "2026-05-08T10:00:05Z",
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
    });
    render(
      <MemoryRouter>
        <ToolObservationPanel run={run} />
      </MemoryRouter>
    );
    expect(screen.getByText("待授权")).toBeDefined();
    // Expand
    await userEvent.click(screen.getByText("web.search"));
    expect(screen.getByText("该工具调用需要用户授权确认。")).toBeDefined();
    expect(screen.getByText("查看权限/提案")).toBeDefined();
  });

  it("handles object with throwing toJSON gracefully", async () => {
    const toxic = {
      data: "safe",
      toJSON() {
        throw new Error("BANG");
      },
    };
    const run = makeRun({
      actions: [
        {
          id: "action-toxic",
          actionType: "tool_call",
          status: "succeeded",
          input: {},
          output: toxic,
          timestamp: "2026-05-08T10:00:05Z",
          toolScope: {
            toolId: "test.toxic",
            toolName: "test.toxic",
            source: "builtin",
            riskLevel: "low",
            capabilities: ["read"],
            actionType: "read",
          },
        },
      ],
    });
    render(
      <MemoryRouter>
        <ToolObservationPanel run={run} />
      </MemoryRouter>
    );
    await userEvent.click(screen.getByText("test.toxic"));
    expect(screen.getByText(/data/)).toBeDefined();
  });

  it("tail sentinel inside deeply nested object does not appear in DOM", async () => {
    const tailSentinel = "NESTED_TAIL_SECRET_999";
    const deep: Record<string, unknown> = {};
    let current: Record<string, unknown> = deep;
    for (let i = 0; i < 5; i++) {
      current.child = {};
      current = current.child as Record<string, unknown>;
    }
    current.secret = tailSentinel;
    const run = makeRun({
      actions: [
        {
          id: "action-deep",
          actionType: "tool_call",
          status: "succeeded",
          input: {},
          output: deep,
          timestamp: "2026-05-08T10:00:05Z",
          toolScope: {
            toolId: "test.deep",
            toolName: "test.deep",
            source: "builtin",
            riskLevel: "low",
            capabilities: ["read"],
            actionType: "read",
          },
        },
      ],
    });
    render(
      <MemoryRouter>
        <ToolObservationPanel run={run} />
      </MemoryRouter>
    );
    await userEvent.click(screen.getByText("test.deep"));
    expect(document.body.innerHTML).not.toContain(tailSentinel);
  });

  it("shows estimatedLength truncation text for object output", async () => {
    const largeObj: Record<string, string> = {};
    for (let i = 0; i < 100; i++) {
      largeObj[`key_${i}`] = `val_${i}_padding`;
    }
    const run = makeRun({
      actions: [
        {
          id: "action-estimated",
          actionType: "tool_call",
          status: "succeeded",
          input: {},
          output: largeObj,
          timestamp: "2026-05-08T10:00:05Z",
          toolScope: {
            toolId: "test.estimated",
            toolName: "test.estimated",
            source: "builtin",
            riskLevel: "low",
            capabilities: ["read"],
            actionType: "read",
          },
        },
      ],
    });
    render(
      <MemoryRouter>
        <ToolObservationPanel run={run} />
      </MemoryRouter>
    );
    await userEvent.click(screen.getByText("test.estimated"));
    expect(screen.getByText(/估算/)).toBeDefined();
    expect(screen.getByText(/字符/)).toBeDefined();
  });
});
