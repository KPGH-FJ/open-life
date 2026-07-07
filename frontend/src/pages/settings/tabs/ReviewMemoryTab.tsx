import { Link } from "react-router-dom";
import type { AppConfig, LifeStateProjection } from "../../../tauri";
import { CapabilityCard, StatusChip } from "../../../components/product/ProductPrimitives";
import { mailboxRoute } from "../../../productShellContract";
import { reviewRequiredCountFromProjection } from "../../../utils/lifeStateProjection";

interface ReviewMemoryTabProps {
  config: AppConfig;
  setConfig: React.Dispatch<React.SetStateAction<AppConfig>>;
  projection: LifeStateProjection | null;
}

export default function ReviewMemoryTab({ config, setConfig, projection }: ReviewMemoryTabProps) {
  const proposalEnabled = config.chat_proposal?.enabled ?? true;
  const pendingCount = reviewRequiredCountFromProjection(projection, "settings");
  const highRiskCount = projection?.pending.highRiskReviewRequiredCount ?? 0;

  return (
    <>
      <section className="grid gap-3 md:grid-cols-3">
        <CapabilityCard
          title="Mailbox"
          description="记忆、Life Model 和权限建议在确认前不会生效。"
          tone={pendingCount == null ? "neutral" : pendingCount > 0 ? "warning" : "ready"}
          meta={pendingCount == null ? "pending status loading" : `${pendingCount} pending proposals`}
        >
          <Link
            to={mailboxRoute()}
            className="inline-flex rounded-md bg-stone-900 px-3 py-1.5 text-xs font-semibold text-white hover:bg-stone-800"
          >
            打开 Mailbox
          </Link>
        </CapabilityCard>
        <CapabilityCard
          title="Proposal-first"
          description="长期记忆和 Life Model 更新只能先创建建议。"
          tone="ready"
          meta="强制"
        >
          <StatusChip label="silent write blocked" tone="ready" />
        </CapabilityCard>
        <CapabilityCard
          title="High Risk"
          description="高风险建议需要更谨慎处理，不应批量同意。"
          tone={highRiskCount > 0 ? "danger" : "neutral"}
          meta={`${highRiskCount} 高风险`}
        />
      </section>

      <section className="space-y-4 border-t pt-4">
        <div>
          <h3 className="text-sm font-medium text-gray-700">记忆建议设置</h3>
          <p className="mt-1 text-xs leading-5 text-gray-500">
            Chat 里抽取到的长期记忆只会进入 Mailbox，不会静默写入 Life Model 或长期记忆。
          </p>
        </div>
        <div className="grid gap-4">
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={proposalEnabled}
              onChange={e =>
                setConfig(prev => ({
                  ...prev,
                  chat_proposal: {
                    ...prev.chat_proposal,
                    enabled: e.target.checked,
                  },
                }))
              }
              className="rounded border-gray-300"
            />
            <span className="text-sm text-gray-700">启用对话中的记忆建议</span>
          </label>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="mb-1 block text-xs text-gray-500">置信度阈值</label>
              <input
                type="number"
                min="0"
                max="1"
                step="0.1"
                value={config.chat_proposal?.confidence_threshold ?? 0.6}
                onChange={e =>
                  setConfig(prev => ({
                    ...prev,
                    chat_proposal: {
                      ...prev.chat_proposal,
                      confidence_threshold: parseFloat(e.target.value),
                    },
                  }))
                }
                className="w-full rounded-lg border border-gray-200 px-3 py-2 text-sm"
              />
            </div>
            <div>
              <label className="mb-1 block text-xs text-gray-500">最小消息长度</label>
              <input
                type="number"
                min="5"
                max="100"
                value={config.chat_proposal?.min_message_length ?? 10}
                onChange={e =>
                  setConfig(prev => ({
                    ...prev,
                    chat_proposal: {
                      ...prev.chat_proposal,
                      min_message_length: parseInt(e.target.value),
                    },
                  }))
                }
                className="w-full rounded-lg border border-gray-200 px-3 py-2 text-sm"
              />
            </div>
          </div>

          <div>
            <label className="mb-1 block text-xs text-gray-500">提取冷却时间（秒）</label>
            <input
              type="number"
              min="0"
              max="3600"
              step="60"
              value={config.chat_proposal?.cooldown_seconds ?? 300}
              onChange={e =>
                setConfig(prev => ({
                  ...prev,
                  chat_proposal: {
                    ...prev.chat_proposal,
                    cooldown_seconds: parseInt(e.target.value),
                  },
                }))
              }
              className="w-full rounded-lg border border-gray-200 px-3 py-2 text-sm"
            />
          </div>
        </div>
      </section>
    </>
  );
}
