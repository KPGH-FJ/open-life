import { useEffect, useRef, type ReactNode } from "react";
import { ArrowLeft, PanelRightOpen, Search, Settings, X, type LucideIcon } from "lucide-react";
import {
  FoundationEvidenceRow,
  FoundationIconButton,
  FoundationLiveRegion,
  FoundationNavRow,
  FoundationStatusLabel,
  type FoundationStatus,
} from "@/ui/foundation";

export type WorkbenchShellMode = "product" | "settings";

export interface WorkbenchNavigationItem {
  id: string;
  label: string;
  meta?: string;
  searchTerms?: readonly string[];
  icon: LucideIcon;
  badge?: string;
}

export interface WorkbenchBoundarySummary {
  label: string;
  detail: string;
  status: FoundationStatus;
  verified?: boolean;
}

export interface WorkbenchContextSummary {
  eyebrow?: string;
  title: string;
  status?: {
    label: string;
    status: FoundationStatus;
    verified?: boolean;
  };
}

export interface WorkbenchEvidenceReference {
  id: string;
  label: string;
  source: string;
  sensitivity: string;
}

export interface WorkbenchInspectorModel {
  title: string;
  conclusion: string;
  risk: string;
  nextAction: string;
  evidence: WorkbenchEvidenceReference[];
  evidenceFeedback?: string;
  technicalDetails?: ReadonlyArray<{ label: string; value: string }>;
}

export interface OpenLifeWorkbenchShellProps {
  mode: WorkbenchShellMode;
  activeNavigationId: string;
  navigationItems: readonly WorkbenchNavigationItem[];
  productSidebar?: ReactNode;
  onNavigate: (id: string) => void;
  activeSettingsId: string;
  settingsItems: readonly WorkbenchNavigationItem[];
  settingsQuery: string;
  onSettingsQueryChange: (query: string) => void;
  onSettingsNavigate: (id: string) => void;
  onOpenSettings: () => void;
  onBackFromSettings: () => void;
  boundary: WorkbenchBoundarySummary;
  context: WorkbenchContextSummary;
  focusKey: string;
  inspectorOpen: boolean;
  inspector: WorkbenchInspectorModel;
  inspectorContent?: ReactNode;
  onOpenInspector: () => void;
  onCloseInspector: () => void;
  onOpenEvidence: (evidence: WorkbenchEvidenceReference) => void;
  announcement: string;
  children: ReactNode;
}

export function OpenLifeWorkbenchShell({
  mode,
  activeNavigationId,
  navigationItems,
  productSidebar,
  onNavigate,
  activeSettingsId,
  settingsItems,
  settingsQuery,
  onSettingsQueryChange,
  onSettingsNavigate,
  onOpenSettings,
  onBackFromSettings,
  boundary,
  context,
  focusKey,
  inspectorOpen,
  inspector,
  inspectorContent,
  onOpenInspector,
  onCloseInspector,
  onOpenEvidence,
  announcement,
  children,
}: OpenLifeWorkbenchShellProps) {
  const contextHeadingRef = useRef<HTMLHeadingElement>(null);
  const mainRef = useRef<HTMLElement>(null);
  const inspectorHeadingRef = useRef<HTMLHeadingElement>(null);
  const inspectorTriggerRef = useRef<HTMLButtonElement>(null);
  const settingsTriggerRef = useRef<HTMLButtonElement>(null);
  const previousFocusKeyRef = useRef(focusKey);
  const previousInspectorOpenRef = useRef(inspectorOpen);
  const previousModeRef = useRef(mode);

  useEffect(() => {
    const focusKeyChanged = previousFocusKeyRef.current !== focusKey;
    const inspectorOpened = !previousInspectorOpenRef.current && inspectorOpen;
    const inspectorClosed = previousInspectorOpenRef.current && !inspectorOpen;

    if (focusKeyChanged) {
      contextHeadingRef.current?.focus();
    } else if (inspectorOpened) {
      inspectorHeadingRef.current?.focus();
    } else if (inspectorClosed) {
      inspectorTriggerRef.current?.focus();
    }

    previousFocusKeyRef.current = focusKey;
    previousInspectorOpenRef.current = inspectorOpen;
  }, [focusKey, inspectorOpen]);

  useEffect(() => {
    if (previousModeRef.current === "settings" && mode === "product") {
      settingsTriggerRef.current?.focus();
    }
    previousModeRef.current = mode;
  }, [mode]);

  const normalizedQuery = settingsQuery.trim().toLocaleLowerCase("zh-CN");
  const visibleSettingsItems = normalizedQuery
    ? settingsItems.filter(item =>
        [item.label, item.meta ?? "", ...(item.searchTerms ?? [])]
          .join(" ")
          .toLocaleLowerCase("zh-CN")
          .includes(normalizedQuery)
      )
    : settingsItems;

  return (
    <div
      className="ol-foundation ol-workbench-shell"
      data-shell-mode={mode}
      data-active-navigation={activeNavigationId}
      data-inspector-open={inspectorOpen ? "true" : "false"}
    >
      <a
        className="ol-shell-skip-link"
        href="#ol-shell-main"
        onClick={event => {
          event.preventDefault();
          mainRef.current?.focus();
        }}
      >
        跳到主工作区
      </a>

      <aside className="ol-shell-sidebar" aria-label={mode === "settings" ? "设置导航" : "主导航"}>
        <div className="ol-shell-brand">
          <span className="ol-shell-brand__mark" aria-hidden="true">
            OL
          </span>
          <span className="ol-shell-brand__copy">
            <small>个人工作台</small>
            <strong>OpenLife</strong>
          </span>
        </div>

        {mode === "product" ? (
          (productSidebar ?? (
            <nav className="ol-shell-navigation" aria-label="产品区域">
              {navigationItems.map(item => {
                const Icon = item.icon;
                return (
                  <FoundationNavRow
                    key={item.id}
                    label={item.label}
                    meta={item.meta}
                    icon={<Icon size={19} strokeWidth={1.75} />}
                    badge={item.badge}
                    current={activeNavigationId === item.id}
                    onClick={() => onNavigate(item.id)}
                  />
                );
              })}
            </nav>
          ))
        ) : (
          <div className="ol-shell-settings-navigation">
            <button type="button" className="ol-shell-back" onClick={onBackFromSettings}>
              <ArrowLeft size={18} strokeWidth={1.75} aria-hidden="true" />
              返回工作台
            </button>
            <label className="ol-shell-search">
              <span className="ol-sr-only">搜索设置</span>
              <Search size={17} strokeWidth={1.75} aria-hidden="true" />
              <input
                type="search"
                value={settingsQuery}
                placeholder="搜索设置"
                onChange={event => onSettingsQueryChange(event.target.value)}
              />
            </label>
            <div className="ol-shell-search-status">
              <p role="status" aria-live="polite">
                {normalizedQuery
                  ? `找到 ${visibleSettingsItems.length} 个设置分类`
                  : `共 ${settingsItems.length} 个设置分类`}
              </p>
              {normalizedQuery && (
                <FoundationIconButton
                  label="清除设置搜索"
                  icon={<X size={16} strokeWidth={1.75} aria-hidden="true" />}
                  onClick={() => onSettingsQueryChange("")}
                />
              )}
            </div>
            <nav aria-label="设置分类">
              {visibleSettingsItems.map(item => {
                const Icon = item.icon;
                return (
                  <FoundationNavRow
                    key={item.id}
                    label={item.label}
                    meta={item.meta}
                    icon={<Icon size={19} strokeWidth={1.75} />}
                    current={activeSettingsId === item.id}
                    onClick={() => onSettingsNavigate(item.id)}
                  />
                );
              })}
            </nav>
            {visibleSettingsItems.length === 0 && (
              <p className="ol-shell-search-empty">没有匹配设置。清除搜索可查看全部分类。</p>
            )}
          </div>
        )}

        <div className="ol-shell-sidebar-footer">
          <section className="ol-shell-boundary" aria-label="模型与隐私边界">
            <FoundationStatusLabel
              label={boundary.label}
              status={boundary.status}
              verified={boundary.verified}
            />
            <p>{boundary.detail}</p>
          </section>
          {mode === "product" && (
            <button
              ref={settingsTriggerRef}
              type="button"
              className="ol-shell-utility-button"
              aria-label="设置"
              onClick={onOpenSettings}
            >
              <Settings size={19} strokeWidth={1.75} aria-hidden="true" />
              <span>设置</span>
            </button>
          )}
        </div>
      </aside>

      <header className="ol-shell-context-bar">
        <div className="ol-shell-context-copy">
          {context.eyebrow && <span>{context.eyebrow}</span>}
          <h1 ref={contextHeadingRef} id="ol-shell-context-title" tabIndex={-1}>
            {context.title}
          </h1>
        </div>
        <div className="ol-shell-context-actions">
          {context.status && (
            <FoundationStatusLabel
              label={context.status.label}
              status={context.status.status}
              verified={context.status.verified}
            />
          )}
          <FoundationIconButton
            ref={inspectorTriggerRef}
            label={inspectorOpen ? "详情已打开" : "打开详情"}
            icon={<PanelRightOpen size={18} strokeWidth={1.75} aria-hidden="true" />}
            aria-expanded={inspectorOpen}
            aria-controls="ol-shell-inspector"
            onClick={onOpenInspector}
          />
        </div>
      </header>

      <main
        ref={mainRef}
        id="ol-shell-main"
        className="ol-shell-main"
        aria-labelledby="ol-shell-context-title"
        tabIndex={-1}
      >
        {children}
      </main>

      {inspectorOpen && (
        <aside
          id="ol-shell-inspector"
          className="ol-shell-inspector"
          aria-labelledby="ol-shell-inspector-title"
        >
          <header className="ol-shell-inspector__header">
            <div>
              <span>详情</span>
              <h2 ref={inspectorHeadingRef} id="ol-shell-inspector-title" tabIndex={-1}>
                {inspector.title}
              </h2>
            </div>
            <FoundationIconButton
              label="关闭详情"
              icon={<X size={18} strokeWidth={1.75} aria-hidden="true" />}
              onClick={onCloseInspector}
            />
          </header>

          {inspectorContent ?? (
            <div className="ol-shell-inspector__body">
              <section>
                <h3>发生了什么</h3>
                <p>{inspector.conclusion}</p>
              </section>
              <section>
                <h3>风险</h3>
                <p>{inspector.risk}</p>
              </section>
              <section>
                <h3>下一步</h3>
                <p>{inspector.nextAction}</p>
              </section>
              <section>
                <h3>来源与记录</h3>
                <div className="ol-shell-evidence-list">
                  {inspector.evidence.map(evidence => (
                    <FoundationEvidenceRow
                      key={evidence.id}
                      {...evidence}
                      onOpen={() => onOpenEvidence(evidence)}
                    />
                  ))}
                </div>
                {inspector.evidenceFeedback && (
                  <p className="ol-shell-evidence-feedback">{inspector.evidenceFeedback}</p>
                )}
              </section>
              {inspector.technicalDetails && inspector.technicalDetails.length > 0 && (
                <details className="ol-shell-technical-details">
                  <summary>技术检查信息</summary>
                  <dl>
                    {inspector.technicalDetails.map(detail => (
                      <div key={detail.label}>
                        <dt>{detail.label}</dt>
                        <dd>{detail.value}</dd>
                      </div>
                    ))}
                  </dl>
                </details>
              )}
            </div>
          )}
        </aside>
      )}

      <FoundationLiveRegion message={announcement} />
    </div>
  );
}
