import type { ViewModelEnvelope } from "@/tauri";

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
