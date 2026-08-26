import type { ProviderPrivacyBoundarySummary } from "../tauri";

export type SettingsOrchestrationState = {
  phase: "idle" | "dirty" | "saving" | "refreshing_boundary" | "ready" | "unknown" | "failed";
  draftRevision: number;
  savedRevision: number | null;
  providerBoundary: ProviderPrivacyBoundarySummary | null;
  boundaryAppliesToSavedRevision: boolean;
  failureStage: "save" | "boundary_refresh" | null;
  errorCode: string | null;
};

export type SettingsOrchestrationEvent =
  | { type: "reset" }
  | { type: "edit" }
  | { type: "save_requested" }
  | { type: "save_succeeded" }
  | { type: "save_failed"; errorCode: string }
  | { type: "boundary_refresh_retry_requested" }
  | { type: "boundary_refreshed"; boundary: ProviderPrivacyBoundarySummary }
  | { type: "boundary_refresh_failed"; errorCode: string };

export const initialSettingsOrchestrationState: SettingsOrchestrationState = {
  phase: "idle",
  draftRevision: 0,
  savedRevision: 0,
  providerBoundary: null,
  boundaryAppliesToSavedRevision: false,
  failureStage: null,
  errorCode: null,
};

export function settingsOrchestrationReducer(
  state: SettingsOrchestrationState,
  event: SettingsOrchestrationEvent
): SettingsOrchestrationState {
  if (event.type === "reset") {
    return initialSettingsOrchestrationState;
  }
  if (event.type === "edit") {
    return {
      ...state,
      phase: "dirty",
      draftRevision: state.draftRevision + 1,
      failureStage: null,
      errorCode: null,
    };
  }

  if (event.type === "save_requested" && state.phase === "dirty") {
    return { ...state, phase: "saving", failureStage: null, errorCode: null };
  }
  if (state.phase === "saving" && event.type === "save_succeeded") {
    return {
      ...state,
      phase: "refreshing_boundary",
      savedRevision: state.draftRevision,
      boundaryAppliesToSavedRevision: false,
    };
  }
  if (state.phase === "saving" && event.type === "save_failed") {
    return {
      ...state,
      phase: "failed",
      failureStage: "save",
      errorCode: event.errorCode,
    };
  }

  if (
    state.phase === "unknown" &&
    state.failureStage === "boundary_refresh" &&
    event.type === "boundary_refresh_retry_requested"
  ) {
    return {
      ...state,
      phase: "refreshing_boundary",
      failureStage: null,
      errorCode: null,
    };
  }

  if (state.phase === "refreshing_boundary" && event.type === "boundary_refreshed") {
    const boundaryKnown =
      event.boundary.routeType !== "unknown" &&
      event.boundary.externalTransmission !== "unknown" &&
      event.boundary.risk !== "unknown";
    return {
      ...state,
      phase: boundaryKnown ? "ready" : "unknown",
      providerBoundary: event.boundary,
      boundaryAppliesToSavedRevision: true,
      failureStage: null,
      errorCode: null,
    };
  }
  if (state.phase === "refreshing_boundary" && event.type === "boundary_refresh_failed") {
    return {
      ...state,
      phase: "unknown",
      providerBoundary: null,
      boundaryAppliesToSavedRevision: false,
      failureStage: "boundary_refresh",
      errorCode: event.errorCode,
    };
  }

  return state;
}
