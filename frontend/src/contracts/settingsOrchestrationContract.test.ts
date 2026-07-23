import { describe, expect, it } from "vitest";

import type { ProviderPrivacyBoundarySummary } from "../tauri";
import {
  initialSettingsOrchestrationState,
  settingsOrchestrationReducer,
} from "./settingsOrchestrationContract";

function boundary(
  overrides: Partial<ProviderPrivacyBoundarySummary> = {}
): ProviderPrivacyBoundarySummary {
  return {
    routeType: "local",
    externalTransmission: "not_sent",
    providerLabel: "Local provider",
    modelLabel: "Local model",
    privacyLabel: "Local route",
    risk: "low",
    localOnlyRequired: true,
    evidenceRefs: [],
    ...overrides,
  };
}

describe("settings orchestration contract", () => {
  it("keeps connection tests separate from save state", () => {
    const dirty = settingsOrchestrationReducer(initialSettingsOrchestrationState, {
      type: "edit",
    });
    const testing = settingsOrchestrationReducer(dirty, { type: "test_requested" });
    const tested = settingsOrchestrationReducer(testing, {
      type: "test_succeeded",
      result: { ok: true, message: "Connection succeeded" },
    });

    expect(tested.phase).toBe("tested");
    expect(tested.savedRevision).toBe(0);
    expect(tested.draftRevision).toBe(1);
    expect(tested.boundaryAppliesToSavedRevision).toBe(false);
  });

  it("requires a boundary refresh after save before reporting ready", () => {
    const dirty = settingsOrchestrationReducer(initialSettingsOrchestrationState, {
      type: "edit",
    });
    const saving = settingsOrchestrationReducer(dirty, { type: "save_requested" });
    const refreshing = settingsOrchestrationReducer(saving, { type: "save_succeeded" });
    const ready = settingsOrchestrationReducer(refreshing, {
      type: "boundary_refreshed",
      boundary: boundary(),
    });

    expect(refreshing.phase).toBe("refreshing_boundary");
    expect(refreshing.boundaryAppliesToSavedRevision).toBe(false);
    expect(ready.phase).toBe("ready");
    expect(ready.boundaryAppliesToSavedRevision).toBe(true);
  });

  it("keeps unknown or failed privacy refresh fail-closed", () => {
    const dirty = settingsOrchestrationReducer(initialSettingsOrchestrationState, {
      type: "edit",
    });
    const saving = settingsOrchestrationReducer(dirty, { type: "save_requested" });
    const refreshing = settingsOrchestrationReducer(saving, { type: "save_succeeded" });
    const unknown = settingsOrchestrationReducer(refreshing, {
      type: "boundary_refreshed",
      boundary: boundary({
        routeType: "unknown",
        externalTransmission: "unknown",
        risk: "unknown",
      }),
    });
    const failed = settingsOrchestrationReducer(refreshing, {
      type: "boundary_refresh_failed",
      errorCode: "provider_boundary_unavailable",
    });

    expect(unknown.phase).toBe("unknown");
    expect(unknown.boundaryAppliesToSavedRevision).toBe(true);
    expect(failed).toMatchObject({
      phase: "unknown",
      boundaryAppliesToSavedRevision: false,
      failureStage: "boundary_refresh",
    });

    const retrying = settingsOrchestrationReducer(failed, {
      type: "boundary_refresh_retry_requested",
    });
    const recovered = settingsOrchestrationReducer(retrying, {
      type: "boundary_refreshed",
      boundary: boundary(),
    });
    expect(retrying.phase).toBe("refreshing_boundary");
    expect(recovered).toMatchObject({
      phase: "ready",
      boundaryAppliesToSavedRevision: true,
      failureStage: null,
    });
  });

  it("does not treat a failed connection test as a saveable state", () => {
    const dirty = settingsOrchestrationReducer(initialSettingsOrchestrationState, {
      type: "edit",
    });
    const testing = settingsOrchestrationReducer(dirty, { type: "test_requested" });
    const failed = settingsOrchestrationReducer(testing, {
      type: "test_failed",
      errorCode: "connection_refused",
    });
    const unchanged = settingsOrchestrationReducer(failed, { type: "save_requested" });

    expect(unchanged).toEqual(failed);
    expect(unchanged.phase).toBe("failed");
  });
});
