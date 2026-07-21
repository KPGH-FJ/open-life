import {
  getDailyGoals,
  getLifeStateProjection,
  getProviderPrivacyBoundarySummary,
  getTasksViewModel,
  type ProviderPrivacyBoundarySummary,
  type TasksViewModel,
  type ViewModelEnvelope,
  type ViewModelStatus,
} from "@/tauri";
import { journeyErrorCode as errorMessage } from "@/ui/journeys/journeyError";
import {
  buildTodayViewModelEnvelope,
  type BuildTodayViewModelEnvelopeInput,
} from "@/viewmodels/today/todayViewModelAdapter";
import type { TodayViewModelEnvelope } from "@/viewmodels/today/todayViewModel";

export type ReadSourceDiagnostic = {
  id: "life_state_projection" | "daily_goals" | "provider_privacy" | "tasks_view_model";
  status: "loaded" | "failed";
  message?: string;
};

export type TodayReadOnlySnapshot = {
  envelope: TodayViewModelEnvelope;
  boundaryEnvelope: ViewModelEnvelope<ProviderPrivacyBoundarySummary>;
  diagnostics: ReadSourceDiagnostic[];
};

export type TasksReadOnlySnapshot = {
  envelope: ViewModelEnvelope<TasksViewModel>;
  boundaryEnvelope: ViewModelEnvelope<ProviderPrivacyBoundarySummary>;
  diagnostics: ReadSourceDiagnostic[];
};

export interface ReadOnlySpineDataSource {
  loadToday(): Promise<TodayReadOnlySnapshot>;
  loadTasks(): Promise<TasksReadOnlySnapshot>;
}

export function buildReadModelErrorEnvelope<T>(
  targetRef: string,
  code: string,
  message: string
): ViewModelEnvelope<T> {
  return {
    data: null,
    status: "error",
    lastUpdatedAt: null,
    source: "backend-readmodel",
    evidenceRefs: [],
    warnings: [{ code, message, severity: "error", evidenceRefs: [] }],
    actions: {
      primary: [
        {
          id: `${targetRef}.refresh`,
          label: `Refresh ${targetRef}`,
          kind: "refresh",
          enabled: true,
          targetRef,
        },
      ],
      review: [],
      debugOnly: [],
    },
  };
}

function boundaryFailure(error: unknown): ViewModelEnvelope<ProviderPrivacyBoundarySummary> {
  return buildReadModelErrorEnvelope(
    "provider_privacy_boundary",
    "provider_privacy_boundary.load_failed",
    `ProviderPrivacyBoundarySummary could not be loaded: ${errorMessage(error)}`
  );
}

function boundaryNeedsDegradedTodayStatus(
  envelope: ViewModelEnvelope<ProviderPrivacyBoundarySummary>
): boolean {
  return envelope.status === "error" || envelope.status === "stale" || envelope.data === null;
}

function appendTodaySourceWarnings(
  envelope: TodayViewModelEnvelope,
  warnings: Array<{ code: string; message: string }>
): TodayViewModelEnvelope {
  if (warnings.length === 0) return envelope;
  return {
    ...envelope,
    warnings: [
      ...(envelope.warnings ?? []),
      ...warnings.map(warning => ({
        ...warning,
        severity: "warning" as const,
        evidenceRefs: [],
      })),
    ],
  };
}

async function loadTodayFromTauri(): Promise<TodayReadOnlySnapshot> {
  const [projectionResult, dailyGoalsResult, boundaryResult] = await Promise.allSettled([
    getLifeStateProjection(),
    getDailyGoals(),
    getProviderPrivacyBoundarySummary(),
  ]);

  const diagnostics: ReadSourceDiagnostic[] = [
    projectionResult.status === "fulfilled"
      ? { id: "life_state_projection", status: "loaded" }
      : {
          id: "life_state_projection",
          status: "failed",
          message: errorMessage(projectionResult.reason),
        },
    dailyGoalsResult.status === "fulfilled"
      ? { id: "daily_goals", status: "loaded" }
      : {
          id: "daily_goals",
          status: "failed",
          message: errorMessage(dailyGoalsResult.reason),
        },
    boundaryResult.status === "fulfilled"
      ? { id: "provider_privacy", status: "loaded" }
      : {
          id: "provider_privacy",
          status: "failed",
          message: errorMessage(boundaryResult.reason),
        },
  ];

  const boundaryEnvelope =
    boundaryResult.status === "fulfilled"
      ? boundaryResult.value
      : boundaryFailure(boundaryResult.reason);
  const sourceWarnings: Array<{ code: string; message: string }> = [];

  if (projectionResult.status === "rejected") {
    return {
      envelope: buildTodayViewModelEnvelope({
        projection: null,
        dailyGoals: dailyGoalsResult.status === "fulfilled" ? dailyGoalsResult.value : [],
        providerPrivacyBoundary: boundaryEnvelope.data,
        status: "error",
        errorMessage: `LifeStateProjection could not be loaded: ${errorMessage(
          projectionResult.reason
        )}`,
      }),
      boundaryEnvelope,
      diagnostics,
    };
  }

  let requestedStatus: ViewModelStatus | undefined;
  if (dailyGoalsResult.status === "rejected") {
    requestedStatus = "stale";
    sourceWarnings.push({
      code: "today.daily_goals_load_failed",
      message: `Daily goals could not be loaded: ${errorMessage(dailyGoalsResult.reason)}`,
    });
  }
  if (boundaryNeedsDegradedTodayStatus(boundaryEnvelope)) {
    requestedStatus = "stale";
    sourceWarnings.push({
      code: "today.provider_privacy_boundary_unavailable",
      message: "Provider/privacy boundary is unavailable; Today remains fail-closed.",
    });
  }

  const input: BuildTodayViewModelEnvelopeInput = {
    projection: projectionResult.value,
    dailyGoals: dailyGoalsResult.status === "fulfilled" ? dailyGoalsResult.value : [],
    providerPrivacyBoundary: boundaryEnvelope.data,
    status: requestedStatus,
  };

  return {
    envelope: appendTodaySourceWarnings(buildTodayViewModelEnvelope(input), sourceWarnings),
    boundaryEnvelope,
    diagnostics,
  };
}

async function loadTasksFromTauri(): Promise<TasksReadOnlySnapshot> {
  const [tasksResult, boundaryResult] = await Promise.allSettled([
    getTasksViewModel(),
    getProviderPrivacyBoundarySummary(),
  ]);
  const boundaryEnvelope =
    boundaryResult.status === "fulfilled"
      ? boundaryResult.value
      : boundaryFailure(boundaryResult.reason);
  const envelope =
    tasksResult.status === "fulfilled"
      ? tasksResult.value
      : buildReadModelErrorEnvelope<TasksViewModel>(
          "tasks",
          "tasks_view_model.load_failed",
          `TasksViewModel could not be loaded: ${errorMessage(tasksResult.reason)}`
        );

  return {
    envelope,
    boundaryEnvelope,
    diagnostics: [
      tasksResult.status === "fulfilled"
        ? { id: "tasks_view_model", status: "loaded" }
        : {
            id: "tasks_view_model",
            status: "failed",
            message: errorMessage(tasksResult.reason),
          },
      boundaryResult.status === "fulfilled"
        ? { id: "provider_privacy", status: "loaded" }
        : {
            id: "provider_privacy",
            status: "failed",
            message: errorMessage(boundaryResult.reason),
          },
    ],
  };
}

export const tauriReadOnlySpineDataSource: ReadOnlySpineDataSource = {
  loadToday: loadTodayFromTauri,
  loadTasks: loadTasksFromTauri,
};
