import { describe, expect, it } from "vitest";
import { journeyErrorCode } from "./journeyError";

describe("journeyErrorCode", () => {
  it("extracts a stable code from a structured Tauri AppError", () => {
    expect(
      journeyErrorCode({
        kind: "Database",
        detail: {
          message: "credential store path and implementation detail",
          hint: "read_only_degraded",
        },
      })
    ).toBe("Database:read_only_degraded");
  });

  it("does not stringify unknown objects into product text", () => {
    expect(journeyErrorCode({ nested: { secret: "must-not-render" } })).toBe(
      "structured_command_error"
    );
    expect(journeyErrorCode({ message: "private path must-not-render" })).toBe(
      "structured_command_error"
    );
  });

  it("preserves explicit frontend error codes", () => {
    expect(journeyErrorCode(new Error("builder_review_read_model_missing"))).toBe(
      "builder_review_read_model_missing"
    );
  });
});
