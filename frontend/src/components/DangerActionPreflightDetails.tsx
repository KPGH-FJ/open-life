import type { DangerActionPreflightView } from "../tauri";

function preflightValue(value: boolean): string {
  return value ? "是" : "否";
}

function preflightStatus(value: string): string {
  return value.replace(/_/g, " ");
}

export default function DangerActionPreflightDetails({
  view,
}: {
  view: DangerActionPreflightView;
}) {
  const affectedCount =
    view.actionType === "data_import_overwrite" && view.affectedItemCount === 0
      ? "未在预检阶段枚举（以已校验备份为准）"
      : String(view.affectedItemCount);
  const rows = [
    ["风险等级", preflightStatus(view.riskTier)],
    ["写入 durable state", preflightValue(view.writesDurableState)],
    ["影响数量", affectedCount],
    ["id / scope digest", view.affectedItemDigest],
    ["external provider", preflightStatus(view.externalTransmission)],
    ["dry run", preflightValue(view.dryRunAvailable)],
    ["backup / rollback", preflightStatus(view.backupStatus)],
    ["Safe Mode", view.safeModeBlocked ? "blocked" : "未阻断"],
  ];

  return (
    <div className="space-y-3">
      <p>{view.scopeSummary}</p>
      <div className="grid gap-2 text-xs sm:grid-cols-2">
        {rows.map(([label, value]) => (
          <div key={label} className="rounded-md border border-stone-200 bg-white px-2.5 py-2">
            <div className="font-medium text-stone-500">{label}</div>
            <div className="mt-0.5 break-all font-semibold text-stone-900">{value}</div>
          </div>
        ))}
      </div>
      <div>
        <div className="text-xs font-medium text-stone-500">数据类别</div>
        <div className="mt-1 flex flex-wrap gap-1.5">
          {view.dataCategories.map(category => (
            <span
              key={category}
              className="rounded-full border border-slate-200 bg-slate-50 px-2 py-0.5 text-xs font-medium text-slate-700"
            >
              {preflightStatus(category)}
            </span>
          ))}
        </div>
      </div>
      {view.confirmationRequired && view.confirmationPhrase && (
        <div className="rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800">
          最终执行需要输入固定确认短语。
        </div>
      )}
      {view.safeModeBlocked && (
        <div className="rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800">
          Safe Mode 已阻断最终执行入口
          {view.blockingReasons.length ? `：${view.blockingReasons.join(" / ")}` : "。"}
        </div>
      )}
      <div>
        <div className="text-xs font-medium text-stone-500">source refs</div>
        <div className="mt-1 space-y-1">
          {view.sourceRefs.map(ref => (
            <code
              key={ref}
              className="block rounded bg-stone-100 px-2 py-1 text-[11px] text-stone-700"
            >
              {ref}
            </code>
          ))}
        </div>
      </div>
    </div>
  );
}
