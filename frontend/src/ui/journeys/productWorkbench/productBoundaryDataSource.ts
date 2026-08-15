import {
  getProviderPrivacyBoundarySummary,
  type ProviderPrivacyBoundarySummary,
  type ViewModelEnvelope,
} from "@/tauri";
import { journeyErrorCode as errorMessage } from "@/ui/journeys/journeyError";

export interface ProductBoundaryDataSource {
  loadBoundary(): Promise<ViewModelEnvelope<ProviderPrivacyBoundarySummary>>;
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

async function loadBoundaryFromTauri(): Promise<ViewModelEnvelope<ProviderPrivacyBoundarySummary>> {
  try {
    return await getProviderPrivacyBoundarySummary();
  } catch (error) {
    return buildReadModelErrorEnvelope(
      "provider_privacy_boundary",
      "provider_privacy_boundary.load_failed",
      `ProviderPrivacyBoundarySummary could not be loaded: ${errorMessage(error)}`
    );
  }
}

export const tauriProductBoundaryDataSource: ProductBoundaryDataSource = {
  loadBoundary: loadBoundaryFromTauri,
};
