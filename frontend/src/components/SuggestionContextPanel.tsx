import type { ReactNode } from "react";
import { Info, ShieldCheck, Sparkles } from "lucide-react";

interface BadgeItem {
  label: string;
  tone?: "neutral" | "indigo" | "green" | "amber" | "rose" | "blue";
}

interface Props {
  title?: string;
  reason: string;
  affectedPath?: string;
  sourceLabel?: string;
  confidence?: number;
  riskLabel?: string;
  note?: string;
  badges?: BadgeItem[];
  footer?: ReactNode;
}

const toneClassMap: Record<NonNullable<BadgeItem["tone"]>, string> = {
  neutral: "bg-gray-100 text-gray-600",
  indigo: "bg-indigo-50 text-indigo-700",
  green: "bg-green-50 text-green-700",
  amber: "bg-amber-50 text-amber-700",
  rose: "bg-rose-50 text-rose-700",
  blue: "bg-blue-50 text-blue-700",
};

function confidenceTone(confidence: number): BadgeItem["tone"] {
  if (confidence >= 0.8) return "green";
  if (confidence >= 0.6) return "amber";
  return "rose";
}

export default function SuggestionContextPanel({
  title = "为什么会有这个建议",
  reason,
  affectedPath,
  sourceLabel,
  confidence,
  riskLabel,
  note,
  badges = [],
  footer,
}: Props) {
  const metaBadges: BadgeItem[] = [...badges];
  if (sourceLabel) {
    metaBadges.unshift({ label: `来源：${sourceLabel}`, tone: "neutral" });
  }
  if (riskLabel) {
    metaBadges.push({ label: riskLabel, tone: riskLabel.includes("高") ? "rose" : riskLabel.includes("中") ? "amber" : "green" });
  }
  if (typeof confidence === "number") {
    metaBadges.push({
      label: `置信度 ${Math.round(confidence * 100)}%`,
      tone: confidenceTone(confidence),
    });
  }

  return (
    <div className="rounded-lg border border-slate-200 bg-slate-50/80 px-3 py-3 space-y-2.5">
      <div className="flex items-center gap-2 text-xs font-medium text-slate-700">
        <Sparkles size={13} className="text-indigo-500" />
        <span>{title}</span>
      </div>
      {metaBadges.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {metaBadges.map((badge) => (
            <span
              key={badge.label}
              className={`rounded-full px-2 py-0.5 text-[10px] font-medium ${toneClassMap[badge.tone ?? "neutral"]}`}
            >
              {badge.label}
            </span>
          ))}
        </div>
      )}
      <div className="text-xs text-slate-600 leading-relaxed">
        <span className="font-medium text-slate-700">原因：</span>
        {reason}
      </div>
      {affectedPath && (
        <div className="text-[11px] text-slate-500 font-mono">
          影响字段：{affectedPath}
        </div>
      )}
      {note && (
        <div className="rounded-md bg-white/80 px-2.5 py-2 text-[11px] text-slate-500 leading-relaxed">
          <Info size={12} className="inline mr-1 text-slate-400" />
          {note}
        </div>
      )}
      {footer && (
        <div className="rounded-md border border-indigo-100 bg-indigo-50/70 px-2.5 py-2 text-[11px] text-indigo-700">
          <ShieldCheck size={12} className="inline mr-1" />
          {footer}
        </div>
      )}
    </div>
  );
}
