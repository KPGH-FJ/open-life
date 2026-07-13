import { Link } from "react-router-dom";
import type { LifeStateProjection, MemoryViewModel } from "../../../tauri";
import { CapabilityCard, StatusChip } from "../../../components/product/ProductPrimitives";
import { mailboxRoute } from "../../../productShellContract";
import { reviewRequiredCountFromProjection } from "../../../utils/lifeStateProjection";

interface ReviewMemoryTabProps {
  projection: LifeStateProjection | null;
  memoryViewModel: MemoryViewModel | null;
}

export default function ReviewMemoryTab({ projection, memoryViewModel }: ReviewMemoryTabProps) {
  const pendingCount = reviewRequiredCountFromProjection(projection, "settings");
  const highRiskCount = projection?.pending.highRiskReviewRequiredCount ?? 0;
  const memorySummary = memoryViewModel?.summary ?? null;
  const lifecycle = memoryViewModel?.lifecycleSummary ?? null;

  return (
    <>
      <section className="grid gap-3 md:grid-cols-3">
        <CapabilityCard
          title="Mailbox"
          description="记忆、Life Model 和权限建议在确认前不会生效。"
          tone={pendingCount == null ? "neutral" : pendingCount > 0 ? "warning" : "ready"}
          meta={
            pendingCount == null ? "pending status loading" : `${pendingCount} pending proposals`
          }
        >
          <Link
            to={mailboxRoute()}
            className="inline-flex rounded-md bg-stone-900 px-3 py-1.5 text-xs font-semibold text-white hover:bg-stone-800"
          >
            打开 Mailbox
          </Link>
        </CapabilityCard>
        <CapabilityCard
          title="MemoryViewModel"
          description="记忆物化、回滚和待审阅数量来自后台生命周期读模型。"
          tone={memoryViewModel ? "ready" : "warning"}
          meta={memoryViewModel ? "backend" : "unknown"}
        >
          <div className="flex flex-wrap gap-1.5">
            <StatusChip label={`${memorySummary?.materializedCount ?? 0} materialized`} />
            <StatusChip
              label={`${memorySummary?.pendingMaterializationCount ?? 0} pending apply`}
              tone={(memorySummary?.pendingMaterializationCount ?? 0) > 0 ? "warning" : "ready"}
            />
          </div>
        </CapabilityCard>
        <CapabilityCard
          title="High Risk"
          description="高风险建议需要更谨慎处理，不应批量同意。"
          tone={highRiskCount > 0 ? "danger" : "neutral"}
          meta={`${highRiskCount} 高风险`}
        />
      </section>

      <section className="grid gap-3 md:grid-cols-4">
        {[
          ["候选", lifecycle?.candidateCount ?? 0],
          ["待确认", memorySummary?.reviewRequiredCount ?? 0],
          ["已确认", lifecycle?.confirmedCount ?? 0],
          ["已回滚", lifecycle?.rolledBackCount ?? 0],
        ].map(([label, value]) => (
          <div key={label} className="rounded-lg border border-stone-200 bg-white px-3 py-3">
            <div className="text-[11px] font-medium text-stone-500">{label}</div>
            <div className="mt-1 text-lg font-semibold text-stone-900">{value}</div>
          </div>
        ))}
      </section>

      <section className="space-y-2 border-t pt-4">
        <div>
          <h3 className="text-sm font-medium text-gray-700">记忆治理</h3>
          <p className="mt-1 text-xs leading-5 text-gray-500">
            对话中的记忆候选由后端 PolicyRouter 路由。需要审阅的项目进入
            Mailbox；可撤销的显式低风险记忆按后端回执显示。
          </p>
        </div>
      </section>
    </>
  );
}
