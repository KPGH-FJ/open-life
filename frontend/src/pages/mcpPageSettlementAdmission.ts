export interface McpPageSettlementAdmissionState {
  lifecycle: "active" | "inactive";
  currentGeneration: number;
}

export type McpPageSettlementAdmissionDecision =
  | { admitted: true; reason: "current_generation" }
  | { admitted: false; reason: "inactive_lifecycle" | "stale_generation" };

export type McpPageReadSettlementDecision = McpPageSettlementAdmissionDecision & {
  settlement: "fulfilled" | "rejected";
};

export function decideMcpPageSettlementAdmission(
  state: Readonly<McpPageSettlementAdmissionState>,
  settledGeneration: number
): McpPageSettlementAdmissionDecision {
  if (state.lifecycle !== "active") {
    return { admitted: false, reason: "inactive_lifecycle" };
  }
  if (state.currentGeneration !== settledGeneration) {
    return { admitted: false, reason: "stale_generation" };
  }
  return { admitted: true, reason: "current_generation" };
}

export async function settleMcpPageRead<T>(
  read: Promise<T>,
  getState: () => Readonly<McpPageSettlementAdmissionState>,
  generation: number,
  onFulfilled: (value: T) => void,
  onRejected: (error: unknown) => void
): Promise<McpPageReadSettlementDecision> {
  return read.then(
    value => {
      const admission = decideMcpPageSettlementAdmission(getState(), generation);
      if (admission.admitted) {
        onFulfilled(value);
      }
      return { ...admission, settlement: "fulfilled" };
    },
    error => {
      const admission = decideMcpPageSettlementAdmission(getState(), generation);
      if (admission.admitted) {
        onRejected(error);
      }
      return { ...admission, settlement: "rejected" };
    }
  );
}
