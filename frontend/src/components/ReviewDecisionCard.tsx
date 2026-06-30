import { useState } from "react";
import { AlertTriangle, ShieldCheck } from "lucide-react";
import { DecisionCard, StatusChip, TechnicalDetails } from "./product/ProductPrimitives";
import type { ReviewDecisionView } from "../utils/reviewDecision";

function riskTone(view: ReviewDecisionView): "neutral" | "warning" | "danger" {
  return view.riskTone;
}

function SourceDetails({ details }: { details: ReviewDecisionView["sourceDetails"] }) {
  const [expanded, setExpanded] = useState(false);
  if (!details.length) return null;

  return (
    <div className="rounded-md border border-stone-200 bg-white px-3 py-2">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="text-xs font-semibold text-stone-700">来源摘录</div>
        <button
          type="button"
          aria-expanded={expanded}
          onClick={() => setExpanded(current => !current)}
          className="inline-flex h-7 items-center rounded-md border border-stone-200 bg-stone-50 px-2 text-xs font-semibold text-stone-700 hover:bg-stone-100"
        >
          {expanded ? "收起" : "展开"}
        </button>
      </div>
      {expanded && (
        <div data-testid="review-expanded-source-details" className="mt-2 space-y-2">
          {details.map((detail, index) => (
            <div
              key={`${index}-${detail.label}`}
              className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2"
            >
              <div className="text-[11px] font-semibold text-stone-500">{detail.label}</div>
              <div className="mt-1 whitespace-pre-wrap break-words text-sm leading-6 text-stone-800">
                {detail.value}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export default function ReviewDecisionCard({ view }: { view: ReviewDecisionView }) {
  return (
    <div className="space-y-4">
      <div data-testid="review-primary-surface" className="space-y-4">
        <DecisionCard
          eyebrow={view.groupLabel}
          title={view.title}
          description={view.subtitle}
          tone={riskTone(view)}
        >
          <div className="flex flex-wrap gap-2">
            <StatusChip label={view.riskLabel} tone={riskTone(view)} />
            <StatusChip label={`把握 ${view.confidenceLabel}`} tone="info" />
            <StatusChip label={`来源 ${view.sourceLabel}`} />
          </div>
        </DecisionCard>

        <DecisionCard title="变化对比" description="先看变化本身，再决定是否同意。">
          <div className="overflow-hidden rounded-md border border-stone-200 bg-white">
            <div className="hidden grid-cols-[minmax(90px,0.8fr)_minmax(0,1fr)_minmax(0,1fr)] border-b border-stone-100 bg-stone-50 px-3 py-2 text-xs font-semibold text-stone-500 sm:grid">
              <div>字段</div>
              <div>当前值</div>
              <div>将变为</div>
            </div>
            {view.beforeAfter.map(row => (
              <div
                key={row.field}
                className="grid gap-2 border-b border-stone-100 px-3 py-3 text-sm last:border-b-0 sm:grid-cols-[minmax(90px,0.8fr)_minmax(0,1fr)_minmax(0,1fr)] sm:gap-3 sm:py-2"
              >
                <div className="min-w-0">
                  <div className="text-[11px] font-semibold text-stone-400 sm:hidden">字段</div>
                  <div className="break-words font-medium text-stone-700">{row.field}</div>
                </div>
                <div className="min-w-0">
                  <div className="text-[11px] font-semibold text-stone-400 sm:hidden">当前值</div>
                  <div className="break-words text-stone-700">{row.before}</div>
                </div>
                <div className="min-w-0">
                  <div className="text-[11px] font-semibold text-stone-400 sm:hidden">将变为</div>
                  <div className="break-words text-stone-950">{row.after}</div>
                </div>
                {row.redacted && (
                  <div className="text-xs text-stone-500 sm:col-span-3">
                    该字段可能包含敏感或原始内容，主面板只显示摘要。
                  </div>
                )}
              </div>
            ))}
          </div>
        </DecisionCard>

        <DecisionCard title="为什么问你" description={view.why} />

        <DecisionCard
          title="依据"
          description="这些是 OpenLife 用来形成建议的摘要，不是原始私密内容。"
        >
          <div className="space-y-2">
            {view.evidence.map((line, index) => (
              <div
                key={`${index}-${line}`}
                className="flex items-start gap-2 rounded-md border border-sky-100 bg-sky-50 px-3 py-2 text-sm text-sky-950"
              >
                <ShieldCheck
                  size={14}
                  className="mt-0.5 shrink-0 text-sky-700"
                  aria-hidden="true"
                />
                <span>{line}</span>
              </div>
            ))}
          </div>
        </DecisionCard>

        <DecisionCard title="来源摘要" description={view.sourceSummary}>
          <SourceDetails details={view.sourceDetails} />
        </DecisionCard>

        <DecisionCard title="影响与风险" description={view.impactScope} tone={riskTone(view)}>
          <div className="flex items-start gap-2 rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-900">
            <AlertTriangle size={14} className="mt-0.5 shrink-0" aria-hidden="true" />
            <span>未同意前不会写入长期记忆、Life Model、外部文件或工具权限。</span>
          </div>
        </DecisionCard>
      </div>

      <TechnicalDetails>
        <div className="grid gap-2 md:grid-cols-2">
          {view.technicalRows.map(row => (
            <div key={`${row.label}-${row.value}`} className="min-w-0">
              <span className="text-stone-400">{row.label}：</span>
              {row.href ? (
                <a className="break-all text-stone-900 underline" href={row.href}>
                  {row.value}
                </a>
              ) : (
                <span className="break-all">{row.value}</span>
              )}
            </div>
          ))}
        </div>
      </TechnicalDetails>
    </div>
  );
}
