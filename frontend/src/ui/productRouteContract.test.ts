import { describe, expect, it } from "vitest";
import {
  isRetiredProductPath,
  productPath,
  resolveProductionRoute,
  SETTINGS_ROUTE_PATH,
} from "./productRouteContract";

describe("production product route contract", () => {
  it("maps only the canonical desktop product surfaces", () => {
    expect(productPath("today")).toBe("/today");
    expect(productPath("workspace")).toBe("/workspace");
    expect(productPath("tasks")).toBe("/tasks");
    expect(productPath("review")).toBe("/review");
    expect(productPath("life-model")).toBe("/life-model");
  });

  it("keeps Settings outside primary navigation while preserving a return surface", () => {
    expect(resolveProductionRoute(SETTINGS_ROUTE_PATH, "workspace")).toEqual({
      mode: "settings",
      surface: "workspace",
      path: "/settings",
    });
  });

  it("does not redirect retired or unknown routes into a different product surface", () => {
    expect(resolveProductionRoute("/companion")).toBeNull();
    expect(resolveProductionRoute("/runs/task-1")).toBeNull();
    expect(resolveProductionRoute("/unknown")).toBeNull();
    expect(isRetiredProductPath("/companion")).toBe(true);
    expect(isRetiredProductPath("/runs/task-1")).toBe(true);
    expect(isRetiredProductPath("/unknown")).toBe(false);
  });
});
