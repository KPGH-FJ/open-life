import { Link } from "react-router-dom";
import { BoundaryChip, StatusChip, TechnicalDetails } from "./product/ProductPrimitives";
import type { RuntimeDisclosureView } from "../utils/runtimeDisclosure";
import { runDetailRoute } from "../productShellContract";

export default function RuntimeDisclosureStrip({
  view,
  runId,
  compact = false,
}: {
  view: RuntimeDisclosureView;
  runId?: string | null;
  compact?: boolean;
}) {
  return (
    <div className="rounded-lg border border-stone-200 bg-stone-50 px-3 py-2 text-xs text-stone-700">
      <div className="flex flex-wrap items-center gap-1.5">
        <BoundaryChip label={view.boundaryLabel} tone={view.boundaryTone} />
        <StatusChip label={view.routeLabel} tone={view.routeTone} />
        <StatusChip label={view.outcomeLabel} tone={view.outcomeTone} />
        <StatusChip label={view.toolsLabel} />
        <StatusChip
          label={view.proposalsLabel}
          tone={view.proposalsLabel.startsWith("待确认") ? "warning" : "neutral"}
        />
        <StatusChip
          label={view.blockersLabel}
          tone={view.blockersLabel.startsWith("阻断") ? "danger" : "neutral"}
        />
      </div>
      {!compact && (
        <div className="mt-2 grid gap-1 leading-5 text-stone-600 sm:grid-cols-2">
          <div>下一步：{view.nextActionLabel}</div>
          {view.memoryLabel && <div>{view.memoryLabel}</div>}
          {view.routeReason && <div className="sm:col-span-2">路线原因：{view.routeReason}</div>}
          {view.fallbackReason && (
            <div className="sm:col-span-2 text-amber-700">Fallback：{view.fallbackReason}</div>
          )}
        </div>
      )}
      <div className="mt-2 flex flex-wrap items-center gap-2">
        {runId && (
          <Link
            to={runDetailRoute(runId)}
            className="font-semibold text-stone-900 underline-offset-4 hover:underline"
          >
            查看 Runs 详情
          </Link>
        )}
        <TechnicalDetails summary="运行技术详情">
          {view.technicalRows.map(row => (
            <div key={row.label} className="grid gap-1 sm:grid-cols-[120px_minmax(0,1fr)]">
              <span className="font-semibold text-stone-700">{row.label}</span>
              <span className="break-words text-stone-600">{row.value}</span>
            </div>
          ))}
        </TechnicalDetails>
      </div>
    </div>
  );
}
