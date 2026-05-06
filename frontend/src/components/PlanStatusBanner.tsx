import { useEffect, useState } from "react";
import { FileText, RotateCcw, XCircle, Play } from "lucide-react";
import type { PlanOperationResult, AgentPlan } from "../types";
import { listAgentPlansForRun, cancelAgentPlan, retryAgentPlan, continueAgentPlan } from "../tauri";

interface Props {
  runId: string;
  onOperation: (result: PlanOperationResult) => void;
}

export default function PlanStatusBanner({ runId, onOperation }: Props) {
  const [plan, setPlan] = useState<AgentPlan | null>(null);
  const [result, setResult] = useState<PlanOperationResult | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    listAgentPlansForRun(runId)
      .then(plans => setPlan(plans[0] ?? null))
      .catch(() => setPlan(null));
  }, [runId]);

  async function doOperation(op: "cancel" | "retry" | "continue") {
    if (!plan) return;
    const planId = plan.id;
    setLoading(true);
    try {
      let res: PlanOperationResult;
      if (op === "cancel") res = await cancelAgentPlan(planId);
      else if (op === "retry") res = await retryAgentPlan(planId);
      else res = await continueAgentPlan(planId);
      setResult(res);
      onOperation(res);
    } catch {
      setResult({
        planId: plan?.id ?? "",
        operation: op,
        success: false,
        status: "draft",
        deviations: [],
        message: "operation failed",
      });
    }
    setLoading(false);
    listAgentPlansForRun(runId)
      .then(plans => setPlan(plans[0] ?? null))
      .catch(() => {});
  }

  if (!plan) return null;

  const showCancel = ["published", "confirmed", "executing"].includes(plan.status);
  const showRetry = ["failed", "failed_review"].includes(plan.status);
  const showContinue = ["executing"].includes(plan.status);

  const hasActions = showCancel || showRetry || showContinue;

  return (
    <div className="border-b border-slate-200 bg-slate-50 px-4 py-2">
      <div className="flex items-center gap-2 text-xs">
        <FileText size={14} className="text-slate-500" />
        <span className="font-medium text-slate-700">Plan</span>
        <span className="text-slate-500">
          {plan.goal.slice(0, 40)}
          {plan.goal.length > 40 ? "…" : ""}
        </span>
        <span
          className={`ml-auto rounded px-1.5 py-0.5 text-[10px] font-medium ${
            plan.status === "completed"
              ? "bg-green-100 text-green-700"
              : plan.status === "failed" || plan.status === "failed_review"
                ? "bg-red-100 text-red-700"
                : plan.status === "cancelled"
                  ? "bg-gray-100 text-gray-600"
                  : "bg-blue-100 text-blue-700"
          }`}
        >
          {plan.status}
        </span>
      </div>
      {hasActions && (
        <div className="mt-1.5 flex gap-2">
          {showCancel && (
            <button
              onClick={() => doOperation("cancel")}
              disabled={loading}
              className="flex items-center gap-1 rounded border border-red-200 bg-white px-2 py-0.5 text-[11px] text-red-600 hover:bg-red-50"
            >
              <XCircle size={12} /> Cancel
            </button>
          )}
          {showRetry && (
            <button
              onClick={() => doOperation("retry")}
              disabled={loading}
              className="flex items-center gap-1 rounded border border-amber-200 bg-white px-2 py-0.5 text-[11px] text-amber-600 hover:bg-amber-50"
            >
              <RotateCcw size={12} /> Retry
            </button>
          )}
          {showContinue && (
            <button
              onClick={() => doOperation("continue")}
              disabled={loading}
              className="flex items-center gap-1 rounded border border-emerald-200 bg-white px-2 py-0.5 text-[11px] text-emerald-600 hover:bg-emerald-50"
            >
              <Play size={12} /> Continue
            </button>
          )}
        </div>
      )}
      {result && (
        <div className={`mt-1 text-[10px] ${result.success ? "text-green-600" : "text-red-600"}`}>
          {result.operation}: {result.message || (result.success ? "ok" : "failed")}
        </div>
      )}
    </div>
  );
}
