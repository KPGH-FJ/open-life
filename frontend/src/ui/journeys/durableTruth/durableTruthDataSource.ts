import {
  getLifeModelViewModel,
  getMemoryViewModel,
  getReviewCenterViewModel,
  type LifeModelViewModel,
  type MemoryViewModel,
  type ReviewCenterViewModel,
  type ViewModelEnvelope,
} from "@/tauri";
import { journeyErrorCode as errorText } from "@/ui/journeys/journeyError";
import { buildReadModelErrorEnvelope } from "@/ui/journeys/readOnly/readOnlySpineDataSource";

export type DurableTruthDiagnostic = {
  id: "life_model_view_model" | "memory_view_model" | "review_center_view_model";
  status: "loaded" | "failed";
  message?: string;
};

export type DurableTruthSnapshot = {
  lifeModelEnvelope: ViewModelEnvelope<LifeModelViewModel>;
  memoryEnvelope: ViewModelEnvelope<MemoryViewModel>;
  reviewEnvelope: ViewModelEnvelope<ReviewCenterViewModel>;
  diagnostics: DurableTruthDiagnostic[];
};

export interface DurableTruthDataSource {
  loadDurableTruth(): Promise<DurableTruthSnapshot>;
}

function settledEnvelope<T>(
  result: PromiseSettledResult<ViewModelEnvelope<T>>,
  targetRef: string,
  code: string
): ViewModelEnvelope<T> {
  return result.status === "fulfilled"
    ? result.value
    : buildReadModelErrorEnvelope<T>(
        targetRef,
        code,
        `${targetRef} could not be loaded: ${errorText(result.reason)}`
      );
}

export function buildDurableTruthErrorSnapshot(error: unknown): DurableTruthSnapshot {
  const message = errorText(error);
  return {
    lifeModelEnvelope: buildReadModelErrorEnvelope(
      "LifeModelViewModel",
      "life_model_view_model.load_failed",
      `LifeModelViewModel could not be loaded: ${message}`
    ),
    memoryEnvelope: buildReadModelErrorEnvelope(
      "MemoryViewModel",
      "memory_view_model.load_failed",
      `MemoryViewModel could not be loaded: ${message}`
    ),
    reviewEnvelope: buildReadModelErrorEnvelope(
      "ReviewCenterViewModel",
      "review_center_view_model.load_failed",
      `ReviewCenterViewModel could not be loaded: ${message}`
    ),
    diagnostics: [
      { id: "life_model_view_model", status: "failed", message },
      { id: "memory_view_model", status: "failed", message },
      { id: "review_center_view_model", status: "failed", message },
    ],
  };
}

async function loadDurableTruth(): Promise<DurableTruthSnapshot> {
  const [lifeModelResult, memoryResult, reviewResult] = await Promise.allSettled([
    getLifeModelViewModel(),
    getMemoryViewModel(),
    getReviewCenterViewModel(),
  ]);

  return {
    lifeModelEnvelope: settledEnvelope(
      lifeModelResult,
      "LifeModelViewModel",
      "life_model_view_model.load_failed"
    ),
    memoryEnvelope: settledEnvelope(
      memoryResult,
      "MemoryViewModel",
      "memory_view_model.load_failed"
    ),
    reviewEnvelope: settledEnvelope(
      reviewResult,
      "ReviewCenterViewModel",
      "review_center_view_model.load_failed"
    ),
    diagnostics: [
      lifeModelResult.status === "fulfilled"
        ? { id: "life_model_view_model", status: "loaded" }
        : {
            id: "life_model_view_model",
            status: "failed",
            message: errorText(lifeModelResult.reason),
          },
      memoryResult.status === "fulfilled"
        ? { id: "memory_view_model", status: "loaded" }
        : {
            id: "memory_view_model",
            status: "failed",
            message: errorText(memoryResult.reason),
          },
      reviewResult.status === "fulfilled"
        ? { id: "review_center_view_model", status: "loaded" }
        : {
            id: "review_center_view_model",
            status: "failed",
            message: errorText(reviewResult.reason),
          },
    ],
  };
}

export const tauriDurableTruthDataSource: DurableTruthDataSource = {
  loadDurableTruth,
};
