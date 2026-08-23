import { describe, expect, it } from "vitest";
import { productErrorCode, productErrorMessage } from "./productError";

describe("productErrorCode", () => {
  it("extracts a stable code from a structured Tauri AppError", () => {
    expect(
      productErrorCode({
        kind: "Database",
        detail: {
          message: "credential store path and implementation detail",
          hint: "read_only_degraded",
        },
      })
    ).toBe("Database:read_only_degraded");
  });

  it("does not stringify unknown objects into product text", () => {
    expect(productErrorCode({ nested: { secret: "must-not-render" } })).toBe(
      "structured_command_error"
    );
    expect(productErrorCode({ message: "private path must-not-render" })).toBe(
      "structured_command_error"
    );
  });

  it("uses an explicit backend code without exposing its private message", () => {
    expect(
      productErrorCode({
        kind: "Internal",
        detail: {
          message: "private backend details must-not-render",
          code: "canonical_steering_checkpoint_passed",
        },
      })
    ).toBe("canonical_steering_checkpoint_passed");
  });

  it("preserves explicit frontend error codes", () => {
    expect(productErrorCode(new Error("builder_review_read_model_missing"))).toBe(
      "builder_review_read_model_missing"
    );
  });

  it("maps known runtime failures to actionable product language", () => {
    expect(productErrorMessage("provider_reasoning_without_final_content")).toBe(
      "模型没有返回完整结果。你可以重试本轮工作。"
    );
    expect(productErrorMessage("provider_quota_exhausted")).toContain("不会静默切换");
    expect(productErrorMessage("work_semantic_verification_stalled")).toContain(
      "没有把未经支持的内容标记为完成"
    );
    expect(productErrorMessage("work_plan_json_invalid")).toContain("可靠的执行计划");
    expect(productErrorMessage("artifact_target_precondition_changed")).toContain(
      "目标文件在确认前发生了变化"
    );
    expect(productErrorMessage("web_search_challenge_detected")).toContain("人机验证");
    expect(productErrorMessage("web_search_no_structured_results")).toContain(
      "没有返回可核验的结果"
    );
    expect(productErrorMessage("work_source_grounding_result_invalid")).toContain("当前来源支持");
    expect(productErrorMessage("web_artifact_source_validation_failed")).toContain(
      "读取的来源不足"
    );
    expect(productErrorMessage("web_fetch_distinct_search_result_missing")).toContain(
      "读取的来源不足"
    );
    expect(productErrorMessage("web_artifact_citation_not_in_body")).toContain("正文陈述附近");
    expect(productErrorMessage("work_run_budget_exhausted")).toContain("执行记录仍会保留");
  });

  it("never exposes an unknown internal code on the product surface", () => {
    expect(productErrorMessage("some_new_internal_error_code")).toBe(
      "操作没有完成。你可以重试，或在详情中查看技术信息。"
    );
  });
});
