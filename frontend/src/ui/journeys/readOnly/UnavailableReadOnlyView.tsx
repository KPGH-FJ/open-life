import { ArrowLeft, LayoutList } from "lucide-react";
import { FoundationActionButton, FoundationNotice } from "@/ui/foundation";

export function UnavailableReadOnlyView({
  title,
  reason,
  onToday,
  onTasks,
}: {
  title: string;
  reason: string;
  onToday: () => void;
  onTasks: () => void;
}) {
  return (
    <article className="ol-readonly-page" data-testid="unavailable-product-view">
      <header className="ol-readonly-page-heading">
        <span>当前版本暂不可用</span>
        <h2>{title}</h2>
        <p>{reason}</p>
      </header>
      <FoundationNotice title="此区域暂不可用" tone="neutral">
        所需后端流程尚未接入；当前不会显示替代数据或可执行动作。
      </FoundationNotice>
      <section className="ol-readonly-action-area" aria-label="可用替代入口">
        <div>
          <span>可用替代</span>
          <h3>返回已接入的只读页面</h3>
        </div>
        <div className="ol-readonly-action-row">
          <FoundationActionButton
            label="返回今日"
            variant="primary"
            icon={<ArrowLeft size={18} strokeWidth={1.75} aria-hidden="true" />}
            data-action-category="product"
            data-action-id="unavailable.return_today"
            data-action-kind="open"
            data-action-enabled="true"
            data-action-disabled-reason=""
            data-action-target-ref="today"
            onClick={onToday}
          />
          <FoundationActionButton
            label="查看任务"
            variant="secondary"
            icon={<LayoutList size={18} strokeWidth={1.75} aria-hidden="true" />}
            data-action-category="product"
            data-action-id="unavailable.open_tasks"
            data-action-kind="open"
            data-action-enabled="true"
            data-action-disabled-reason=""
            data-action-target-ref="tasks"
            onClick={onTasks}
          />
        </div>
      </section>
    </article>
  );
}
