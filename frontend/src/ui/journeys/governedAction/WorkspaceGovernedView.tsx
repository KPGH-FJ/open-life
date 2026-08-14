import { Eye, RefreshCw } from "lucide-react";
import { FoundationActionButton, FoundationNotice } from "@/ui/foundation";
import type { GovernedActionSnapshot } from "./governedActionDataSource";
import { WorkspaceConversationPanel } from "./WorkspaceConversationPanel";
import type { WorkspaceConversationController } from "./useWorkspaceConversation";

export function WorkspaceGovernedView({
  snapshot,
  refreshing,
  onRefresh,
  onOpenInspector,
  onOpenLifeModel,
  conversation,
}: {
  snapshot: GovernedActionSnapshot | null;
  refreshing: boolean;
  onRefresh: () => void;
  onOpenInspector: () => void;
  onOpenLifeModel: (itemRef: string) => void;
  conversation?: WorkspaceConversationController;
}) {
  const envelope = snapshot?.workspaceEnvelope;
  const model = envelope && ["ready", "stale"].includes(envelope.status) ? envelope.data : null;
  const activeTask = model?.activeTask;
  const selectedConversationId = conversation?.selectedSessionId ?? null;
  const projectionMatchesConversation =
    !conversation ||
    (model?.selectedConversationId ?? null) === selectedConversationId ||
    (model?.selectedConversationId === "" && selectedConversationId === null);
  const conversationDisabledReason = (() => {
    if (!conversation) return undefined;
    const boundary = model?.providerPrivacyBoundarySummary;
    if (
      boundary?.localOnlyRequired &&
      boundary.routeType !== "local" &&
      boundary.routeType !== "unknown"
    ) {
      return "当前要求仅本机处理，但后端没有确认本地路由。";
    }
    if (activeTask?.lifecycleStatus === "waiting_permission") {
      return "当前 Work 正在等待一个精确决定；处理后可在同一对话继续。";
    }
    return undefined;
  })();

  return (
    <section className="ol-governed-page ol-conversation-workbench" aria-label="Conversation">
      {(!snapshot || !envelope || envelope.status === "loading") && (
        <FoundationNotice title="正在读取 Work 状态" tone="neutral">
          <p>Conversation 历史可以独立读取；Work 状态确认前不显示任务结论。</p>
        </FoundationNotice>
      )}

      {envelope?.status === "error" && (
        <FoundationNotice title="Work 状态暂时不可用" tone="error" live>
          <p>后端没有返回可确认的 canonical Work 状态；普通 Chat 仍可继续。</p>
          <div className="ol-governed-inline-actions">
            <FoundationActionButton
              label="重新读取 Work 状态"
              icon={<RefreshCw size={17} aria-hidden="true" />}
              loading={refreshing}
              loadingLabel="正在读取"
              onClick={onRefresh}
            />
            <FoundationActionButton
              label="查看状态依据"
              icon={<Eye size={17} aria-hidden="true" />}
              variant="quiet"
              onClick={onOpenInspector}
            />
          </div>
        </FoundationNotice>
      )}

      {envelope?.status === "stale" && (
        <FoundationNotice title="Work 状态已陈旧" tone="protection" live>
          <p>当前仍显示上次确认的内容；决定与任务控制保持关闭，直到重新读取成功。</p>
        </FoundationNotice>
      )}

      {model && !projectionMatchesConversation && (
        <FoundationNotice title="正在切换 Conversation 上下文" tone="neutral">
          <p>Work 投影尚未匹配当前 Conversation；旧任务不会暂时显示在这里。</p>
        </FoundationNotice>
      )}

      {conversation && (
        <WorkspaceConversationPanel
          controller={conversation}
          onOpenLifeModel={onOpenLifeModel}
          disabledReason={conversationDisabledReason}
        />
      )}
    </section>
  );
}
