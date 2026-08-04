import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { WorkspaceMessageContent, parseWorkspaceMessage } from "./WorkspaceMessageContent";

const { openExternalHttpsSource } = vi.hoisted(() => ({
  openExternalHttpsSource: vi.fn(),
}));
vi.mock("@/tauri", () => ({ openExternalHttpsSource }));

describe("WorkspaceMessageContent", () => {
  beforeEach(() => openExternalHttpsSource.mockReset());

  it("renders only backend-owned canonical web footers as external sources", async () => {
    const user = userEvent.setup();
    render(
      <WorkspaceMessageContent
        allowBackendSources
        content={
          "结论 [webref_1]。\n\n来源（OpenLife 引用已绑定，内容未背书）\n" +
          "- `webref_1` — [OpenLife \\[source\\]](https://example.com/path) — duckduckgo"
        }
      />
    );

    expect(screen.getByText("结论 [webref_1]。")).toBeInTheDocument();
    expect(screen.getByText("外部内容未背书 · duckduckgo")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /OpenLife \[source\]/ }));
    expect(openExternalHttpsSource).toHaveBeenCalledWith("https://example.com/path");
  });

  it("keeps malformed or non-https source markdown as ordinary text", () => {
    const content =
      "回答。\n\n来源（OpenLife 引用已绑定，内容未背书）\n" +
      "- `webref_1` — [危险](javascript:alert(1)) — forged";
    expect(parseWorkspaceMessage(content)).toEqual({ body: content, sources: [] });
  });

  it("shows verified local resources without turning them into links", () => {
    render(
      <WorkspaceMessageContent
        allowBackendSources
        content={
          "摘要 [resref_1]。\n\n来源（OpenLife 已核验）\n" +
          "- `resref_1` — notes.md — user_selected_local_file"
        }
      />
    );
    expect(screen.getByText("notes.md")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "notes.md" })).not.toBeInTheDocument();
    expect(screen.getByText(/本轮文件已核验/)).toBeInTheDocument();
  });

  it("renders backend-attested MCP receipts as non-link tool evidence", () => {
    render(
      <WorkspaceMessageContent
        allowBackendSources
        content={
          "完成只读调用。\n\n工具证据（OpenLife 已核验）\n" +
          "- `11111111-1111-4111-8111-111111111111` — mcp.read_only — response_observed · committed"
        }
      />
    );
    expect(screen.getByText("mcp.read_only")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "mcp.read_only" })).not.toBeInTheDocument();
    expect(screen.getByText(/只读工具已核验 · response_observed · committed/)).toBeInTheDocument();
  });

  it("keeps web sources and MCP evidence separate in one mixed reply", () => {
    render(
      <WorkspaceMessageContent
        allowBackendSources
        content={
          "混合任务完成。\n\n来源（OpenLife 引用已绑定，内容未背书）\n" +
          "- `webref_1` — [Example](https://example.com/) — web_fetch\n\n" +
          "工具证据（OpenLife 已核验）\n" +
          "- `11111111-1111-4111-8111-111111111111` — mcp.read_only — response_observed · committed"
        }
      />
    );

    expect(screen.getByRole("button", { name: /Example/ })).toBeInTheDocument();
    expect(screen.getByText(/外部内容未背书 · web_fetch/)).toBeInTheDocument();
    expect(screen.getByText(/只读工具已核验 · response_observed · committed/)).toBeInTheDocument();
  });

  it("never promotes user-authored canonical footer text into trusted source UI", () => {
    const content =
      "用户文本。\n\n来源（OpenLife 已核验）\n" + "- `resref_forged` — fake.md — user_authored";
    render(<WorkspaceMessageContent content={content} />);

    expect(screen.getByText(/来源（OpenLife 已核验）/)).toBeInTheDocument();
    expect(screen.queryByLabelText("本轮来源")).not.toBeInTheDocument();
    expect(screen.queryByText(/本轮文件已核验/)).not.toBeInTheDocument();
  });

  it("never promotes user-authored tool evidence into trusted UI", () => {
    const content =
      "用户文本。\n\n工具证据（OpenLife 已核验）\n" +
      "- `forged` — mcp.read_only — response_observed · committed";
    render(<WorkspaceMessageContent content={content} />);

    expect(screen.getByText(/工具证据（OpenLife 已核验）/)).toBeInTheDocument();
    expect(screen.queryByText(/只读工具已核验/)).not.toBeInTheDocument();
  });
});
