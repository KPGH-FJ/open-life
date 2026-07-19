import { useCallback, useState } from "react";
import {
  Check,
  FileText,
  LayoutPanelLeft,
  ListChecks,
  LockKeyhole,
  RefreshCw,
  RotateCcw,
  Settings2,
  ShieldCheck,
  TriangleAlert,
} from "lucide-react";
import {
  FoundationActionButton,
  FoundationDialog,
  FoundationEvidenceRow,
  FoundationIconButton,
  FoundationLiveRegion,
  FoundationNavRow,
  FoundationNotice,
  FoundationStatusLabel,
  FoundationTextField,
  FoundationToggle,
} from "@/ui/foundation";

const HARNESS_MARKER = "OPENLIFE_PHASE4B_DEV_HARNESS";

const sections = [
  { id: "actions", label: "动作", icon: ListChecks },
  { id: "status", label: "状态", icon: ShieldCheck },
  { id: "forms", label: "表单", icon: Settings2 },
  { id: "navigation", label: "导航", icon: LayoutPanelLeft },
] as const;

type SectionId = (typeof sections)[number]["id"];

function scrollToSection(id: SectionId): void {
  document.getElementById(`foundation-${id}`)?.scrollIntoView({ block: "start" });
}

export function FoundationHarness() {
  const [activeSection, setActiveSection] = useState<SectionId>("actions");
  const [dialogOpen, setDialogOpen] = useState(false);
  const [approved, setApproved] = useState(false);
  const [networkPolicy, setNetworkPolicy] = useState<"on" | "off">("off");
  const [feedback, setFeedback] = useState("UI Foundation 已载入。所有内容均为布局样例。");

  const closeDialog = useCallback(() => setDialogOpen(false), []);

  function selectSection(id: SectionId): void {
    setActiveSection(id);
    setFeedback(`已切换到${sections.find(section => section.id === id)?.label}状态组。`);
    scrollToSection(id);
  }

  function resetHarness(): void {
    setActiveSection("actions");
    setDialogOpen(false);
    setApproved(false);
    setNetworkPolicy("off");
    setFeedback("样例状态已重置；没有写入任何产品数据。");
    scrollToSection("actions");
  }

  function approveFixture(): void {
    setApproved(true);
    setDialogOpen(false);
    setFeedback("样例决定已记录；尚未应用，也未写入长期状态。");
  }

  return (
    <div
      className="ol-foundation phase4b-harness"
      data-harness-marker={HARNESS_MARKER}
      data-foundation-dialog-background
    >
      <header className="phase4b-qa-bar">
        <div className="phase4b-qa-identity">
          <span className="phase4b-qa-mark" aria-hidden="true">
            OL
          </span>
          <span>
            <strong>UI Foundation</strong>
            <small>Phase 4B · DEV ONLY</small>
          </span>
        </div>
        <div className="phase4b-qa-flags" aria-label="样例边界">
          <FoundationStatusLabel label="LAYOUT_FIXTURE" />
          <FoundationStatusLabel label="不接产品后端" status="unknown" />
        </div>
        <FoundationIconButton
          label="重置样例状态"
          icon={<RotateCcw size={18} aria-hidden="true" />}
          onClick={resetHarness}
        />
      </header>

      <div className="phase4b-lab">
        <aside className="phase4b-lab-nav" aria-label="组件状态组">
          <div className="phase4b-lab-nav__heading">状态矩阵</div>
          <nav>
            {sections.map(section => {
              const Icon = section.icon;
              return (
                <FoundationNavRow
                  key={section.id}
                  label={section.label}
                  meta="基础组件"
                  icon={<Icon size={18} />}
                  current={activeSection === section.id}
                  onClick={() => selectSection(section.id)}
                />
              );
            })}
          </nav>
          <div className="phase4b-lab-nav__boundary">
            <LockKeyhole size={17} aria-hidden="true" />
            <span>
              <strong>Release 隔离</strong>
              <small>该入口不进入产品 bundle</small>
            </span>
          </div>
        </aside>

        <main className="phase4b-work-surface" id="phase4b-main" tabIndex={-1}>
          <div className="phase4b-page-heading">
            <div>
              <span className="phase4b-eyebrow">OpenLife semantic primitives</span>
              <h1>白色工作台视觉基础</h1>
              <p>固定字号、固定间距、低圆角与有边界的状态颜色。</p>
            </div>
            <FoundationStatusLabel
              label={approved ? "已批准，尚未应用" : "等待样例决定"}
              status={approved ? "waiting" : "neutral"}
            />
          </div>

          <FoundationNotice title="未知状态保持关闭" tone="protection">
            Provider / Privacy 证据缺失时不显示绿色本地结论，也不开放外部动作。
          </FoundationNotice>

          <section id="foundation-actions" className="phase4b-section">
            <div className="phase4b-section-heading">
              <div>
                <span className="phase4b-eyebrow">ActionButton</span>
                <h2>动作与结果边界</h2>
              </div>
              <span>default · hover · focus · disabled · loading</span>
            </div>
            <div className="phase4b-action-grid">
              <FoundationActionButton
                label="批准样例"
                variant="primary"
                icon={<Check size={18} aria-hidden="true" />}
                onClick={() => setDialogOpen(true)}
              />
              <FoundationActionButton
                label="查看依据"
                variant="secondary"
                icon={<FileText size={18} aria-hidden="true" />}
                onClick={() => setFeedback("已打开样例依据摘要；没有改变任何产品状态。")}
              />
              <FoundationActionButton
                label="稍后处理"
                variant="quiet"
                onClick={() => setFeedback("样例已标记为稍后处理；当前决定仍未完成。")}
              />
              <FoundationActionButton
                label="拒绝样例"
                variant="danger"
                onClick={() => setFeedback("样例已拒绝；没有发生读取、写入或外部传输。")}
              />
              <FoundationActionButton
                label="应用变更"
                disabled
                disabledReason="缺少后端应用命令；批准不能显示为已完成。"
              />
              <FoundationActionButton
                label="刷新状态"
                loading
                loadingLabel="正在刷新"
                icon={<RefreshCw size={18} aria-hidden="true" />}
              />
            </div>
          </section>

          <section id="foundation-status" className="phase4b-section">
            <div className="phase4b-section-heading">
              <div>
                <span className="phase4b-eyebrow">Status + Notice</span>
                <h2>语义状态</h2>
              </div>
              <span>绿色只用于已验证成功</span>
            </div>
            <div className="phase4b-status-row">
              <FoundationStatusLabel label="默认" />
              <FoundationStatusLabel label="等待确认" status="waiting" />
              <FoundationStatusLabel label="陈旧" status="stale" />
              <FoundationStatusLabel label="未知" status="unknown" />
              <FoundationStatusLabel label="已阻断" status="blocked" />
              <FoundationStatusLabel label="具体错误" status="error" />
              <FoundationStatusLabel label="已验证成功" status="success" verified />
            </div>
            <div className="phase4b-notice-grid">
              <FoundationNotice title="安全模式正在保护当前状态" tone="protection">
                可以继续查看和整理，但外部动作与长期写入保持关闭。
              </FoundationNotice>
              <FoundationNotice title="外部结果未知" tone="error">
                可能产生副作用的请求没有返回终态。刷新证据前不要重试。
              </FoundationNotice>
            </div>
          </section>

          <section id="foundation-forms" className="phase4b-section">
            <div className="phase4b-section-heading">
              <div>
                <span className="phase4b-eyebrow">Field + Toggle</span>
                <h2>设置控件</h2>
              </div>
              <span>测试、保存、边界刷新相互独立</span>
            </div>
            <div className="phase4b-form-grid">
              <FoundationTextField
                id="phase4b-provider-endpoint"
                label="供应商地址"
                description="布局样例，不会发起连接。"
                defaultValue="https://api.example.invalid/v1"
              />
              <FoundationTextField
                id="phase4b-provider-secret"
                label="访问凭据"
                description="密钥值不进入搜索或证据正文。"
                type="password"
                defaultValue="fixture-secret"
                stateMessage="仅用于控件布局验证"
              />
              <FoundationTextField
                id="phase4b-provider-error"
                label="模型名称"
                defaultValue=""
                error="模型名称为空；测试连接保持不可用。"
              />
              <div className="phase4b-toggle-stack">
                <FoundationToggle
                  label="允许样例网络策略"
                  description="切换只更新本地 QA 状态，不代表真实授权。"
                  state={networkPolicy}
                  onChange={next => {
                    setNetworkPolicy(next);
                    setFeedback(`样例网络策略已切换为${next === "on" ? "开" : "关"}。`);
                  }}
                />
                <FoundationToggle
                  label="当前传输边界"
                  description="未知不能表现为关闭或本地。"
                  state="unknown"
                />
              </div>
            </div>
          </section>

          <section id="foundation-navigation" className="phase4b-section">
            <div className="phase4b-section-heading">
              <div>
                <span className="phase4b-eyebrow">Nav + Evidence</span>
                <h2>导航和证据入口</h2>
              </div>
              <span>所有可点击控件均有可见结果</span>
            </div>
            <div className="phase4b-nav-evidence-grid">
              <div className="phase4b-nav-sample" aria-label="导航样例">
                <FoundationNavRow
                  label="今日"
                  meta="当前页面样例"
                  icon={<ListChecks size={18} />}
                  current
                  onClick={() => setFeedback("今日样例已保持为当前导航。")}
                />
                <FoundationNavRow
                  label="任务"
                  meta="尚未迁移"
                  icon={<LayoutPanelLeft size={18} />}
                  badge="不可用"
                  onClick={() => setFeedback("任务页面尚未迁移；没有重定向，也没有改变产品状态。")}
                />
              </div>
              <div className="phase4b-evidence-sample">
                <FoundationEvidenceRow
                  id="evidence_fixture_permission_scope"
                  label="样例权限范围"
                  source="fixture.permission.scope"
                  sensitivity="local_private"
                  onOpen={() => setFeedback("已打开样例权限范围；这是布局证据，不是真实授权。")}
                />
                <FoundationEvidenceRow
                  id="evidence_fixture_read_model_refresh"
                  label="样例刷新结果"
                  source="fixture.read_model.refresh"
                  sensitivity="metadata_only"
                  onOpen={() => setFeedback("已打开样例刷新结果；状态仍由刷新后的读模型决定。")}
                />
              </div>
            </div>
          </section>

          <div className="phase4b-feedback">
            <TriangleAlert size={17} aria-hidden="true" />
            <span>{feedback}</span>
          </div>

          <details className="phase4b-advanced">
            <summary>技术检查信息</summary>
            <dl>
              <div>
                <dt>entry</dt>
                <dd>dev/phase4b/index.html</dd>
              </div>
              <div>
                <dt>fixture</dt>
                <dd>{HARNESS_MARKER}</dd>
              </div>
              <div>
                <dt>backend</dt>
                <dd>not_connected</dd>
              </div>
            </dl>
          </details>
        </main>
      </div>

      <FoundationDialog
        open={dialogOpen}
        title="确认批准布局样例"
        description="该操作只验证对话框、焦点恢复和状态文案。"
        onClose={closeDialog}
        footer={
          <>
            <FoundationActionButton label="取消" variant="secondary" onClick={closeDialog} />
            <FoundationActionButton label="确认批准" variant="primary" onClick={approveFixture} />
          </>
        }
      >
        <FoundationNotice title="批准不等于应用" tone="protection">
          确认后只显示“已批准，尚未应用”。只有刷新后的 applied 读模型才能显示完成。
        </FoundationNotice>
      </FoundationDialog>
      <FoundationLiveRegion message={feedback} />
    </div>
  );
}
