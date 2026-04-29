import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import BuilderPatchReview from "./BuilderPatchReview";
import type { BuilderSignal, BuilderSummary, BuilderSignalDecision } from "../tauri";

const mockSignals: BuilderSignal[] = [
  {
    id: "sig_name",
    source_step: 1,
    source_question_id: "name",
    dimension: "Identity",
    affected_path: "identity.name",
    proposed_value: "小林",
    confidence: 0.95,
    reason: "用户直接提供的称呼",
    risk_level: "low",
    user_status: "Pending",
  },
  {
    id: "sig_focus",
    source_step: 2,
    source_question_id: "current_focus",
    dimension: "State",
    affected_path: "state.current_focus",
    proposed_value: "事业 / 学业",
    confidence: 0.9,
    reason: "用户选择的当前关注主题",
    risk_level: "low",
    user_status: "Pending",
  },
  {
    id: "sig_long_term",
    source_step: 4,
    source_question_id: "long_term_direction",
    dimension: "Goals",
    affected_path: "goals.long_term",
    proposed_value: [{ name: "长期方向: 成为技术专家", priority: 5 }],
    confidence: 0.6,
    reason: "用户描述的长期方向（需要确认）",
    risk_level: "high",
    user_status: "Pending",
  },
];

const mockSummary: BuilderSummary = {
  identity_summary: "基于 1 个信号",
  goals_summary: "基于 1 个信号",
  capabilities_summary: "基于 0 个信号",
  state_summary: "基于 1 个信号",
  assumptions: ["用户通过快速构建流程提供"],
  unresolved_questions: [],
  recommended_next_steps: ["审阅并确认信号", "可选择进入渐进构建继续完善"],
};

describe("BuilderPatchReview", () => {
  const mockApply = vi.fn();
  const mockCreateProposals = vi.fn();
  const mockReject = vi.fn();

  beforeEach(() => {
    mockApply.mockClear();
    mockCreateProposals.mockClear();
    mockReject.mockClear();
  });

  it("renders component with title and signals", () => {
    render(
      <BuilderPatchReview
        signals={mockSignals}
        summary={mockSummary}
        onApply={mockApply}
        onReject={mockReject}
      />
    );

    expect(screen.getByText("OpenLife 准备这样理解你")).toBeInTheDocument();
    expect(screen.getByText("小林")).toBeInTheDocument();
    expect(screen.getByText("长期方向: 成为技术专家")).toBeInTheDocument();
  });

  it("groups signals by dimension", () => {
    render(
      <BuilderPatchReview
        signals={mockSignals}
        summary={mockSummary}
        onApply={mockApply}
        onReject={mockReject}
      />
    );

    expect(screen.getAllByText("Identity 我是谁").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("Goals 我要去哪里").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("State 我现在怎么样").length).toBeGreaterThanOrEqual(1);
  });

  it("shows risk levels correctly", () => {
    render(
      <BuilderPatchReview
        signals={mockSignals}
        summary={mockSummary}
        onApply={mockApply}
        onReject={mockReject}
      />
    );

    expect(screen.getAllByText("低风险").length).toBeGreaterThanOrEqual(2);
    expect(screen.getAllByText("高风险").length).toBeGreaterThanOrEqual(1);
  });

  it("low risk signals are checked by default, high risk are not", () => {
    render(
      <BuilderPatchReview
        signals={mockSignals}
        summary={mockSummary}
        onApply={mockApply}
        onReject={mockReject}
      />
    );

    const checkboxes = screen.getAllByRole("checkbox");
    expect(checkboxes[0]).toBeChecked(); // low risk
    expect(checkboxes[1]).toBeChecked(); // low risk
    expect(checkboxes[2]).not.toBeChecked(); // high risk
  });

  it("toggles signal selection when clicking checkbox", () => {
    render(
      <BuilderPatchReview
        signals={mockSignals}
        summary={mockSummary}
        onApply={mockApply}
        onReject={mockReject}
      />
    );

    const checkboxes = screen.getAllByRole("checkbox");

    // Uncheck a low risk item
    fireEvent.click(checkboxes[0]);
    expect(checkboxes[0]).not.toBeChecked();

    // Check a high risk item
    fireEvent.click(checkboxes[2]);
    expect(checkboxes[2]).toBeChecked();
  });

  it("calls onApply with BuilderSignalDecision[] when clicking save", () => {
    render(
      <BuilderPatchReview
        signals={mockSignals}
        summary={mockSummary}
        onApply={mockApply}
        onReject={mockReject}
      />
    );

    const saveButton = screen.getByText("直接应用（legacy / 绕过 Review Center）");
    fireEvent.click(saveButton);

    // 低风险的已默认选中，高风险的未选中 → rejected
    const decisions: BuilderSignalDecision[] = mockApply.mock.calls[0][0];
    expect(decisions).toHaveLength(3);

    const accepted = decisions.filter(d => d.status === "accepted");
    const rejected = decisions.filter(d => d.status === "rejected");
    expect(accepted).toHaveLength(2);
    expect(rejected).toHaveLength(1);
    expect(accepted.map(d => d.id)).toContain("sig_name");
    expect(accepted.map(d => d.id)).toContain("sig_focus");
    expect(rejected.map(d => d.id)).toContain("sig_long_term");
  });

  it("uses Review Center as the default submission path when available", () => {
    render(
      <BuilderPatchReview
        signals={mockSignals}
        summary={mockSummary}
        onApply={mockApply}
        onCreateProposals={mockCreateProposals}
        onReject={mockReject}
      />
    );

    expect(screen.getByText("发送到 Review Center")).toBeInTheDocument();
    expect(screen.queryByText("直接应用（legacy / 绕过 Review Center）")).not.toBeInTheDocument();

    fireEvent.click(screen.getByText("发送到 Review Center"));

    expect(mockCreateProposals).toHaveBeenCalledTimes(1);
    expect(mockApply).not.toHaveBeenCalled();
  });

  it("calls onReject when clicking reject button", () => {
    render(
      <BuilderPatchReview
        signals={mockSignals}
        summary={mockSummary}
        onApply={mockApply}
        onReject={mockReject}
      />
    );

    const rejectButton = screen.getByText("暂不保存");
    fireEvent.click(rejectButton);

    expect(mockReject).toHaveBeenCalled();
  });

  it("shows high risk warning when high risk items are unchecked", () => {
    render(
      <BuilderPatchReview
        signals={mockSignals}
        summary={mockSummary}
        onApply={mockApply}
        onReject={mockReject}
      />
    );

    expect(screen.getByText(/你有未勾选的高风险字段/)).toBeInTheDocument();
  });

  it("disables save button when no signals selected", () => {
    render(
      <BuilderPatchReview
        signals={[mockSignals[2]]} // Only high risk, unchecked by default
        summary={mockSummary}
        onApply={mockApply}
        onReject={mockReject}
      />
    );

    const saveButton = screen.getByText("直接应用（legacy / 绕过 Review Center）");
    expect(saveButton).toBeDisabled();
  });

  it("shows summary cards with counts", () => {
    render(
      <BuilderPatchReview
        signals={mockSignals}
        summary={mockSummary}
        onApply={mockApply}
        onReject={mockReject}
      />
    );

    // New summary cards show accept/edit/merge/skip/total counts
    expect(screen.getByText("接受")).toBeInTheDocument();
    expect(screen.getByText("编辑")).toBeInTheDocument();
    expect(screen.getByText("跳过")).toBeInTheDocument();
    expect(screen.getByText("总计")).toBeInTheDocument();
    // 2低风险信号默认被选中，所以接受计数应该是2
    // 用getAllByText因为可能有多个"2"（如step number）
    const countElements = screen.getAllByText("2");
    expect(countElements.length).toBeGreaterThanOrEqual(1);
  });

  it("renders a unified suggestion context panel for each signal", () => {
    render(
      <BuilderPatchReview
        signals={mockSignals}
        summary={mockSummary}
        onApply={mockApply}
        onReject={mockReject}
      />
    );

    expect(screen.getAllByText("为什么会有这个建议").length).toBeGreaterThanOrEqual(3);
    expect(screen.getByText("影响字段：identity.name")).toBeInTheDocument();
    expect(screen.getByText("来源：name")).toBeInTheDocument();
  });

  it("allows editing a signal inline and marks it as edited in decisions", async () => {
    render(
      <BuilderPatchReview
        signals={mockSignals}
        summary={mockSummary}
        onApply={mockApply}
        onReject={mockReject}
      />
    );

    // Find edit buttons (pencil icons)
    const editButtons = screen.getAllByTitle("编辑");
    fireEvent.click(editButtons[0]);

    // Should show input field
    const input = screen.getByDisplayValue("小林");
    expect(input).toBeInTheDocument();

    // Edit the value
    fireEvent.change(input, { target: { value: "Alex" } });

    // Save the edit
    const saveEditButton = screen.getByText("保存");
    fireEvent.click(saveEditButton);

    // Click main save button
    const saveButton = screen.getByText("直接应用（legacy / 绕过 Review Center）");
    fireEvent.click(saveButton);

    await waitFor(() => {
      const decisions: BuilderSignalDecision[] = mockApply.mock.calls[0][0];
      const editedDecision = decisions.find(d => d.id === "sig_name");
      expect(editedDecision).toBeDefined();
      expect(editedDecision!.status).toBe("edited");
      expect(editedDecision!.proposed_value).toBe("Alex");
    });
  });

  it("shows assumptions and recommended steps", () => {
    render(
      <BuilderPatchReview
        signals={mockSignals}
        summary={mockSummary}
        onApply={mockApply}
        onReject={mockReject}
      />
    );

    expect(screen.getByText("AI 做出的假设")).toBeInTheDocument();
    expect(screen.getByText("• 用户通过快速构建流程提供")).toBeInTheDocument();
    expect(screen.getByText("建议的下一步")).toBeInTheDocument();
    expect(screen.getByText("• 审阅并确认信号")).toBeInTheDocument();
  });

  it("renders edited proposed value in UI after inline edit", async () => {
    render(
      <BuilderPatchReview
        signals={mockSignals}
        summary={mockSummary}
        onApply={mockApply}
        onReject={mockReject}
      />
    );

    // Edit the first signal
    const editButtons = screen.getAllByTitle("编辑");
    fireEvent.click(editButtons[0]);

    const input = screen.getByDisplayValue("小林");
    fireEvent.change(input, { target: { value: "Alex" } });

    const saveEditButton = screen.getByText("保存");
    fireEvent.click(saveEditButton);

    // UI should display the new edited value
    await waitFor(() => {
      const valueDisplay = screen.getByTestId("proposed-value-sig_name");
      expect(valueDisplay).toHaveTextContent("Alex");
    });
  });

  it("keeps complex edited values as JSON arrays in decisions", async () => {
    render(
      <BuilderPatchReview
        signals={mockSignals}
        summary={mockSummary}
        onApply={mockApply}
        onReject={mockReject}
      />
    );

    const editButtons = screen.getAllByTitle("编辑");
    fireEvent.click(editButtons[2]);

    const textarea = screen.getByDisplayValue(/成为技术专家/);
    fireEvent.change(textarea, {
      target: {
        value: JSON.stringify([{ name: "长期方向: 成为产品型工程师", priority: 9 }], null, 2),
      },
    });
    fireEvent.click(screen.getByText("保存"));
    fireEvent.click(screen.getByText("直接应用（legacy / 绕过 Review Center）"));

    // Confirm direct apply for high-risk signal
    await waitFor(() => {
      expect(screen.getByText("确认直接写入")).toBeInTheDocument();
    });
    fireEvent.click(screen.getByText("确认直接写入"));

    await waitFor(() => {
      const decisions: BuilderSignalDecision[] = mockApply.mock.calls[0][0];
      const editedDecision = decisions.find(d => d.id === "sig_long_term");
      expect(editedDecision).toMatchObject({
        id: "sig_long_term",
        status: "edited",
        proposed_value: [{ name: "长期方向: 成为产品型工程师", priority: 9 }],
      });
    });
  });

  it("blocks invalid JSON edits for complex values", async () => {
    render(
      <BuilderPatchReview
        signals={mockSignals}
        summary={mockSummary}
        onApply={mockApply}
        onReject={mockReject}
      />
    );

    const editButtons = screen.getAllByTitle("编辑");
    fireEvent.click(editButtons[2]);

    const textarea = screen.getByDisplayValue(/成为技术专家/);
    fireEvent.change(textarea, { target: { value: "[invalid json" } });
    fireEvent.click(screen.getByText("保存"));

    expect(await screen.findByText("JSON 格式无效，请修正后再保存。")).toBeInTheDocument();
    expect(mockApply).not.toHaveBeenCalled();
  });

  it("does not include rejected signals in apply payload", async () => {
    render(
      <BuilderPatchReview
        signals={mockSignals}
        summary={mockSummary}
        onApply={mockApply}
        onReject={mockReject}
      />
    );

    // Default: low-risk checked, high-risk unchecked → rejected
    // Explicitly uncheck the first low-risk signal to make it rejected
    const checkboxes = screen.getAllByRole("checkbox");
    fireEvent.click(checkboxes[0]); // uncheck sig_name (low risk)

    const saveButton = screen.getByText("直接应用（legacy / 绕过 Review Center）");
    fireEvent.click(saveButton);

    await waitFor(() => {
      const decisions: BuilderSignalDecision[] = mockApply.mock.calls[0][0];
      expect(decisions).toHaveLength(3);

      const rejected = decisions.filter(d => d.status === "rejected");
      expect(rejected).toHaveLength(2);
      expect(rejected.map(d => d.id)).toContain("sig_name");
      expect(rejected.map(d => d.id)).toContain("sig_long_term");

      // Rejected decisions must NOT contain proposed_value
      const nameRejected = rejected.find(d => d.id === "sig_name");
      expect(nameRejected).not.toHaveProperty("proposed_value");
    });
  });

  it("shows Chinese field path labels", () => {
    render(
      <BuilderPatchReview
        signals={[
          {
            id: "sig_values",
            source_step: 1,
            source_question_id: "q1",
            dimension: "Identity",
            affected_path: "identity.values",
            proposed_value: [{ name: "成长", weight: 5 }],
            confidence: 0.85,
            reason: "用户多次提到",
            risk_level: "low",
            user_status: "Pending",
          },
          {
            id: "sig_mission",
            source_step: 1,
            source_question_id: "q2",
            dimension: "Identity",
            affected_path: "identity.mission_statement",
            proposed_value: "帮助他人成长",
            confidence: 0.75,
            reason: "用户自述",
            risk_level: "medium",
            user_status: "Pending",
          },
        ]}
        summary={mockSummary}
        onApply={mockApply}
        onReject={mockReject}
      />
    );

    expect(screen.getByText("价值观")).toBeInTheDocument();
    expect(screen.getByText("使命宣言")).toBeInTheDocument();
    // Full path still visible in impact preview
    expect(screen.getByText(/identity\.values/)).toBeInTheDocument();
  });

  it("shows confidence bar with color coding", () => {
    render(
      <BuilderPatchReview
        signals={mockSignals}
        summary={mockSummary}
        onApply={mockApply}
        onReject={mockReject}
      />
    );

    // Confidence percentages should be visible
    expect(screen.getAllByText(/置信度 95%/).length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText(/置信度 60%/).length).toBeGreaterThanOrEqual(1);
  });

  it("shows reasoning text for each signal", () => {
    render(
      <BuilderPatchReview
        signals={mockSignals}
        summary={mockSummary}
        onApply={mockApply}
        onReject={mockReject}
      />
    );

    // Multiple signals each have reasoning context panels
    const reasoningLabels = screen.getAllByText(/为什么会有这个建议/);
    expect(reasoningLabels.length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("用户直接提供的称呼")).toBeInTheDocument();
    expect(screen.getByText("用户选择的当前关注主题")).toBeInTheDocument();
  });

  it("shows proposed value badge for each signal", () => {
    render(
      <BuilderPatchReview
        signals={mockSignals}
        summary={mockSummary}
        onApply={mockApply}
        onReject={mockReject}
      />
    );

    // Each signal has a "建议值" badge
    const badges = screen.getAllByText("建议值");
    expect(badges.length).toBeGreaterThanOrEqual(2);
  });
});
