window.addEventListener("DOMContentLoaded", () => {
  const states = window.OPENLIFE_MOCKUP_STATES || [];
  const navItems = window.OPENLIFE_MOCKUP_NAV || [];
  const stateById = new Map(states.map((state) => [state.id, state]));
  const navByKey = new Map(navItems.map((item) => [item.key, item]));
  const mobileQuery = window.matchMedia("(max-width: 980px)");

  let currentStateId = states[0]?.id || null;
  let lastFixtureStateId = currentStateId;
  let currentNavKey = states[0]?.navKey || "today";
  let lastDrawerTrigger = null;
  let lastInspectorTrigger = null;

  const qaStateSelector = document.getElementById("qaStateSelector");
  const workbenchShell = document.querySelector(".workbench-shell");
  const sidebarNav = document.getElementById("sidebarNav");
  const sidebarUtilityNav = document.getElementById("sidebarUtilityNav");
  const mobileDrawerNav = document.getElementById("mobileDrawerNav");
  const mobileUtilityNav = document.getElementById("mobileUtilityNav");
  const mobileBottomNav = document.getElementById("mobileBottomNav");
  const workSurface = document.getElementById("workSurface");
  const inspector = document.getElementById("evidenceInspector");
  const inspectorContent = document.getElementById("inspectorContent");
  const inspectorTitle = document.getElementById("inspectorTitle");
  const inspectorCloseButton = document.getElementById("inspectorCloseButton");
  const mobileEvidenceButton = document.getElementById("mobileEvidenceButton");
  const mobilePrivacyDot = document.getElementById("mobilePrivacyDot");
  const privacyStatusButton = document.getElementById("privacyStatusButton");
  const privacyStatusDot = document.getElementById("privacyStatusDot");
  const privacyStatusLabel = document.getElementById("privacyStatusLabel");
  const supportDetailsButton = document.getElementById("supportDetailsButton");
  const mobileSupportDetailsButton = document.getElementById("mobileSupportDetailsButton");
  const mobileMenuButton = document.getElementById("mobileMenuButton");
  const mobileNavOverlay = document.getElementById("mobileNavOverlay");
  const mobileNavDrawer = document.getElementById("mobileNavDrawer");
  const mobileNavCloseButton = document.getElementById("mobileNavCloseButton");
  const mobileNavScrim = document.getElementById("mobileNavScrim");
  const surfaceTitle = document.getElementById("surfaceTitle");
  const surfaceEyebrow = document.getElementById("surfaceEyebrow");
  const topPrimaryStatus = document.getElementById("topPrimaryStatus");
  const liveRegion = document.getElementById("liveRegion");
  const actionDialog = document.getElementById("actionDialog");
  const actionDialogTitle = document.getElementById("actionDialogTitle");
  const actionDialogSummary = document.getElementById("actionDialogSummary");
  const actionDialogPayload = document.getElementById("actionDialogPayload");
  const actionDialogActions = document.getElementById("actionDialogActions");

  function escapeHtml(value) {
    return String(value)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#039;");
  }

  function icon(name) {
    const icons = {
      calendar: '<path d="M8 2v4M16 2v4M3 9h18M5 4h14a2 2 0 0 1 2 2v13a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2Z" />',
      workspace: '<rect width="18" height="14" x="3" y="4" rx="2" /><path d="M8 21h8M12 18v3" />',
      tasks: '<path d="m9 11 3 3L22 4" /><path d="M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11" />',
      review: '<path d="M20 13c0 5-3.5 7.5-8 9-4.5-1.5-8-4-8-9V5l8-3 8 3v8Z" /><path d="m9 12 2 2 4-5" />',
      lifemodel: '<circle cx="12" cy="8" r="4" /><path d="M4 22a8 8 0 0 1 16 0" />',
      settings: '<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.38a2 2 0 0 0-.73-2.73l-.15-.09a2 2 0 0 1-1-1.74v-.51a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2Z" /><circle cx="12" cy="12" r="3" />',
      terminal: '<path d="m4 17 6-6-6-6" /><path d="M12 19h8" />',
      menu: '<path d="M4 6h16M4 12h16M4 18h16" />',
      close: '<path d="M18 6 6 18M6 6l12 12" />',
    };

    return `
      <svg viewBox="0 0 24 24" aria-hidden="true" data-lucide-name="${escapeHtml(name)}">
        <g fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
          ${icons[name] || icons.workspace}
        </g>
      </svg>
    `;
  }

  function renderStaticIcons(root = document) {
    root.querySelectorAll("[data-static-icon]").forEach((node) => {
      node.innerHTML = icon(node.getAttribute("data-static-icon"));
    });
  }

  function announce(message) {
    liveRegion.textContent = "";
    window.requestAnimationFrame(() => {
      liveRegion.textContent = message;
    });
  }

  function statusLabel(status) {
    return `<span class="status-chip ${escapeHtml(status.tone || "is-muted")}">${escapeHtml(status.label)}</span>`;
  }

  function privacyPresentation(boundary) {
    const hasEvidence = Array.isArray(boundary.evidenceRefs) && boundary.evidenceRefs.length > 0;
    if (
      boundary.routeType === "local" &&
      boundary.externalTransmission === "not_sent" &&
      hasEvidence
    ) {
      return { label: "本地路由已证实", tone: "is-success" };
    }
    if (boundary.externalTransmission === "sent") {
      return { label: "已发生外部传输", tone: "is-danger" };
    }
    if (boundary.externalTransmission === "possible") {
      return { label: "可能外传", tone: "is-warning" };
    }
    return { label: "传输状态未知", tone: "is-warning" };
  }

  function unavailablePrivacyBoundary() {
    return {
      routeType: "unknown",
      externalTransmission: "unknown",
      providerLabel: "provider unknown",
      modelLabel: "model unknown",
      privacyLabel: "该未覆盖页面没有模型与隐私读模型",
      risk: "unknown",
      localOnlyRequired: false,
      blockedReason: "No product read model is represented for this surface.",
      evidenceRefs: [],
    };
  }

  function renderPrivacyBoundary(boundary) {
    const presentation = privacyPresentation(boundary);
    privacyStatusLabel.textContent = presentation.label;
    privacyStatusDot.className = `status-dot ${presentation.tone}`;
    mobilePrivacyDot.className = `mobile-privacy-dot ${presentation.tone}`;
    privacyStatusButton.setAttribute(
      "aria-label",
      `查看模型与隐私依据：${presentation.label}`,
    );
    mobileEvidenceButton.setAttribute(
      "aria-label",
      `打开依据与风险：${presentation.label}`,
    );
  }

  function renderQaSelector(unavailableLabel = null) {
    const unavailableOption = unavailableLabel
      ? `<option value="" selected disabled>当前：${escapeHtml(unavailableLabel)}</option>`
      : "";
    qaStateSelector.innerHTML = `${unavailableOption}${states
      .map(
        (state) =>
          `<option value="${escapeHtml(state.id)}"${state.id === currentStateId ? " selected" : ""}>${escapeHtml(state.shortLabel)}</option>`,
      )
      .join("")}`;
  }

  function navButton(item, context) {
    const active = item.key === currentNavKey;
    const unavailable = Boolean(item.unavailable);
    const unavailableMeta = unavailable ? '<span class="nav-availability">未覆盖</span>' : "";
    const contextClass = context === "bottom" ? " bottom-nav-item" : "";
    return `
      <button
        class="nav-item${contextClass}${active ? " is-active" : ""}"
        type="button"
        data-nav-key="${escapeHtml(item.key)}"
        ${active ? 'aria-current="page"' : ""}
      >
        <span class="nav-glyph" aria-hidden="true">${icon(item.icon)}</span>
        <span class="nav-label">${escapeHtml(item.label)}</span>
        ${unavailableMeta}
      </button>
    `;
  }

  function renderNavigation() {
    const primaryItems = navItems.filter((item) => item.placement === "primary");
    const utilityItems = navItems.filter((item) => item.placement === "utility");
    sidebarNav.innerHTML = primaryItems.map((item) => navButton(item, "sidebar")).join("");
    sidebarUtilityNav.innerHTML = utilityItems
      .map((item) => navButton(item, "sidebar"))
      .join("");
    mobileDrawerNav.innerHTML = primaryItems.map((item) => navButton(item, "drawer")).join("");
    mobileUtilityNav.innerHTML = utilityItems.map((item) => navButton(item, "drawer")).join("");
    mobileBottomNav.innerHTML = primaryItems
      .filter((item) => item.mobilePrimary && !item.unavailable)
      .map((item) => navButton(item, "bottom"))
      .join("");

    document.querySelectorAll("[data-nav-key]").forEach((button) => {
      button.addEventListener("click", (event) => {
        event.preventDefault();
        const item = navByKey.get(button.getAttribute("data-nav-key"));
        if (item) handleNavigation(item, button);
      });
    });
  }

  function actionButton(action, lane, options = {}) {
    const reasonId =
      options.reasonId || `${action.id.replace(/[^a-zA-Z0-9_-]/g, "-")}-reason`;
    const renderReason = options.renderReason !== false;
    const tone = `${lane === "review" ? " is-review" : ""}${
      lane === "review" && action.kind === "approve" ? " is-approval" : ""
    }`;
    const disabled = action.enabled ? "" : " disabled";
    const describedBy = action.enabled ? "" : ` aria-describedby="${reasonId}"`;
    return `
      <div class="action-control">
        <button
          class="action-button${tone}${
            lane === "primary" && action.kind !== "inspect" ? " is-primary" : ""
          }"
          type="button"
          data-action-id="${escapeHtml(action.id)}"
          data-action-lane="${escapeHtml(lane)}"
          ${disabled}${describedBy}
        >${escapeHtml(action.label)}</button>
        ${
          action.enabled || !renderReason
            ? ""
            : `<span class="action-reason" id="${reasonId}">${escapeHtml(action.disabledReason)}</span>`
        }
      </div>
    `;
  }

  function renderActionLane(actions, lane, ariaLabel) {
    if (!actions.length) return "";
    return `
      <div
        class="action-lane"
        data-action-lane="${escapeHtml(lane)}"
        role="group"
        aria-label="${escapeHtml(ariaLabel)}"
      >
        <div class="action-row">${actions.map((action) => actionButton(action, lane)).join("")}</div>
      </div>
    `;
  }

  function renderActions(state) {
    const actions = state.actions;
    return `
      <section class="product-action-area" aria-labelledby="actionAreaTitle">
        <div class="action-area-heading">
          <p class="section-kicker">下一步</p>
          <h3 id="actionAreaTitle">${escapeHtml(state.actionPrompt)}</h3>
        </div>
        <div class="action-lanes">
          ${renderActionLane(actions.primary || [], "primary", "查看与导航")}
          ${renderActionLane(actions.review || [], "review", "你的决定")}
        </div>
      </section>
    `;
  }

  function renderSection(section) {
    return `
      <section class="surface-section">
        <header class="section-heading">
          <p class="section-kicker">${escapeHtml(section.label)}</p>
          <h3>${escapeHtml(section.title)}</h3>
        </header>
        <div class="section-rows">
          ${section.rows
            .map(
              (row) => `
                <div class="list-row ${escapeHtml(row.tone || "")}">
                  <div>
                    <strong>${escapeHtml(row.title)}</strong>
                    <p>${escapeHtml(row.body)}</p>
                  </div>
                  <span class="row-meta">${escapeHtml(row.meta)}</span>
                </div>
              `,
            )
            .join("")}
        </div>
      </section>
    `;
  }

  function renderMetrics(metrics) {
    if (!metrics.length) return "";
    return `
      <div class="metric-cluster" aria-label="当前指标">
        ${metrics
          .map(
            (metric) => `
              <div class="metric-item">
                <span class="metric-value">${escapeHtml(metric.value)}</span>
                <span class="metric-label">${escapeHtml(metric.label)}</span>
              </div>
            `,
          )
          .join("")}
      </div>
    `;
  }

  function renderReviewContext(context) {
    if (!context) return "";
    return `
      <section class="decision-context" aria-labelledby="decisionContextTitle">
        <header class="section-heading">
          <p class="section-kicker">建议内容</p>
          <h3 id="decisionContextTitle">${escapeHtml(context.changeSummary)}</h3>
        </header>
        <div class="change-comparison" aria-label="当前内容与建议内容对比">
          <div class="change-side is-before">
            <span>当前</span>
            <strong>${escapeHtml(context.before)}</strong>
          </div>
          <div class="change-arrow" aria-hidden="true">→</div>
          <div class="change-side is-after">
            <span>建议</span>
            <strong>${escapeHtml(context.after)}</strong>
          </div>
        </div>
        <dl class="decision-facts">
          <div><dt>原因</dt><dd>${escapeHtml(context.reason)}</dd></div>
          <div><dt>来源</dt><dd>${escapeHtml(context.source)}</dd></div>
          <div><dt>风险</dt><dd>${escapeHtml(context.risk)}</dd></div>
          <div><dt>影响</dt><dd>${escapeHtml(context.impact)}</dd></div>
          <div><dt>有效期</dt><dd>${escapeHtml(context.expires)}</dd></div>
        </dl>
      </section>
    `;
  }

  function renderPermissionContext(context) {
    if (!context) return "";
    return `
      <section class="permission-context" aria-labelledby="permissionContextTitle">
        <header class="section-heading">
          <p class="section-kicker">访问请求</p>
          <h3 id="permissionContextTitle">${escapeHtml(context.title)}</h3>
          <p>${escapeHtml(context.purpose)}</p>
        </header>
        <dl class="permission-facts">
          <div><dt>将使用</dt><dd>${escapeHtml(context.tool)}</dd></div>
          <div><dt>访问位置</dt><dd>${escapeHtml(context.target)}</dd></div>
          <div><dt>数据范围</dt><dd>${escapeHtml(context.dataScope)}</dd></div>
          <div><dt>是否外传</dt><dd>${escapeHtml(context.transmission)}</dd></div>
          <div><dt>授权时长</dt><dd>${escapeHtml(context.duration)}</dd></div>
          <div><dt>如何撤销</dt><dd>${escapeHtml(context.revocation)}</dd></div>
        </dl>
      </section>
    `;
  }

  function renderWorkspaceDecisionActions(state) {
    const actions = state.actions.review || [];
    const disabledAction = actions.find((action) => !action.enabled);
    const reasonId = "workspacePermissionDecisionReason";
    return `
      <div
        class="workspace-decision-area"
        data-component="ProductActionArea"
        role="group"
        aria-label="${escapeHtml(state.actionPrompt)}"
      >
        <div class="workspace-decision-heading">
          <p class="section-kicker">下一步</p>
          <h4>${escapeHtml(state.actionPrompt)}</h4>
        </div>
        <div class="action-row workspace-decision-actions">
          ${actions
            .map((action) =>
              actionButton(action, "review", {
                renderReason: false,
                reasonId: action.enabled ? undefined : reasonId,
              }),
            )
            .join("")}
        </div>
        ${
          disabledAction
            ? `<p class="workspace-decision-reason" id="${reasonId}">${escapeHtml(disabledAction.disabledReason)}</p>`
            : ""
        }
      </div>
    `;
  }

  function renderWorkspaceTimelineEvent(event, state) {
    const isPermissionEvent = event.status === "waiting";
    const marker = event.status === "done" ? "✓" : event.status === "waiting" ? "!" : "";
    return `
      <li class="workspace-timeline-event is-${escapeHtml(event.status)}">
        <span class="timeline-marker" aria-hidden="true">${marker}</span>
        <article class="timeline-event-body">
          <header class="timeline-event-heading">
            <div>
              <p class="section-kicker">${escapeHtml(event.label)}</p>
              <h4>${escapeHtml(event.title)}</h4>
            </div>
            <span class="timeline-meta">${escapeHtml(event.meta)}</span>
          </header>
          <p class="timeline-event-copy">${escapeHtml(event.body)}</p>
          ${
            isPermissionEvent
              ? `
                <ul class="permission-summary" aria-label="访问范围摘要">
                  ${state.permissionContext.summaryItems
                    .map(
                      (item) =>
                        `<li class="${escapeHtml(item.tone)}">${escapeHtml(item.label)}</li>`,
                    )
                    .join("")}
                </ul>
                <p class="workspace-fail-closed">${escapeHtml(state.blocker.body)}</p>
                ${renderWorkspaceDecisionActions(state)}
              `
              : ""
          }
        </article>
      </li>
    `;
  }

  function renderWorkspaceSurface(state) {
    const primaryActions = state.actions.primary || [];
    workSurface.innerHTML = `
      <section class="workspace-objective" aria-labelledby="currentGoalTitle">
        <div>
          <p class="section-kicker">${escapeHtml(state.goal.label)}</p>
          <h3 id="currentGoalTitle">${escapeHtml(state.goal.title)}</h3>
          <p>${escapeHtml(state.goal.summary)}</p>
        </div>
        ${renderActionLane(primaryActions, "primary", "任务依据")}
      </section>

      <section class="workspace-timeline" aria-labelledby="workspaceTimelineTitle">
        <header class="workspace-timeline-heading">
          <div>
            <p class="section-kicker">执行记录</p>
            <h3 id="workspaceTimelineTitle">当前任务进度</h3>
          </div>
          <p>一次只展开当前需要处理的事件</p>
        </header>
        <ol class="workspace-timeline-list">
          ${(state.timeline || []).map((event) => renderWorkspaceTimelineEvent(event, state)).join("")}
        </ol>
      </section>
    `;
  }

  function renderWorkSurface(state) {
    workSurface.classList.toggle("has-sticky-decisions", Boolean(state.stickyDecisionActions));
    workSurface.classList.toggle("is-workspace-focus", state.layout === "workspace_timeline");
    if (state.layout === "workspace_timeline") {
      renderWorkspaceSurface(state);
      return;
    }
    workSurface.innerHTML = `
      <section class="objective-panel" aria-labelledby="currentGoalTitle">
        <div>
          <p class="section-kicker">${escapeHtml(state.goal.label)}</p>
          <h3 id="currentGoalTitle">${escapeHtml(state.goal.title)}</h3>
          <p>${escapeHtml(state.goal.summary)}</p>
        </div>
        ${renderMetrics(state.metrics)}
      </section>

      ${renderReviewContext(state.reviewContext)}
      ${renderPermissionContext(state.permissionContext)}

      <section class="semantic-banner ${escapeHtml(state.blocker.tone)}" aria-labelledby="blockerTitle">
        <div>
          <p class="section-kicker">${escapeHtml(state.blocker.label)}</p>
          <strong id="blockerTitle">${escapeHtml(state.blocker.title)}</strong>
        </div>
        <p>${escapeHtml(state.blocker.body)}</p>
      </section>

      ${renderActions(state)}

      ${
        state.sections.length
          ? `<div class="surface-grid">${state.sections.map(renderSection).join("")}</div>`
          : ""
      }
    `;
  }

  function sourceLabel(source) {
    const labels = {
      "backend-readmodel": "后端读模型",
      audit: "审计",
      task: "任务",
      review: "审核",
      memory: "记忆",
      lifemodel: "LifeModel",
      settings: "设置",
      provider: "模型服务",
    };
    return labels[source] || source;
  }

  function sensitivityLabel(sensitivity) {
    const labels = {
      public: "公开",
      local_private: "本地私密",
      sensitive: "敏感",
      redacted: "已脱敏",
    };
    return labels[sensitivity] || sensitivity || "未标注";
  }

  function collectFieldSources(state) {
    const items = [
      { element: "顶部主状态", sourceRef: state.primaryStatus.sourceRef },
      { element: "当前目标", sourceRef: state.goal.sourceRef },
      { element: "阻塞或风险", sourceRef: state.blocker.sourceRef },
      { element: "隐私状态", sourceRef: "ProviderPrivacyBoundarySummary" },
      ...(state.reviewContext
        ? [{ element: "建议决策上下文", sourceRef: state.reviewContext.sourceRef }]
        : []),
      ...(state.permissionContext
        ? [{ element: "权限范围上下文", sourceRef: state.permissionContext.sourceRef }]
        : []),
      ...(state.permissionContext?.summaryItems || []).map((item) => ({
        element: `权限摘要：${item.label}`,
        sourceRef: item.sourceRef,
      })),
      ...state.metrics.map((metric) => ({
        element: `指标：${metric.label}`,
        sourceRef: metric.sourceRef,
      })),
      ...(state.timeline || []).map((event) => ({
        element: `执行事件：${event.title}`,
        sourceRef: event.sourceRef,
      })),
      ...state.sections.flatMap((section) =>
        section.rows.map((row) => ({
          element: `列表：${row.title}`,
          sourceRef: row.sourceRef,
        })),
      ),
      ...(state.actions.primary || []).map((action) => ({
        element: `产品动作：${action.id}`,
        sourceRef: action.sourceRef,
      })),
      ...(state.actions.review || []).map((action) => ({
        element: `审核动作：${action.id}`,
        sourceRef: action.sourceRef,
      })),
      ...(state.actions.debugOnly || []).map((action) => ({
        element: `调试动作：${action.id}`,
        sourceRef: action.sourceRef,
      })),
    ];
    return items;
  }

  function renderInspectorOverview(summary) {
    return `
      <section class="inspector-section inspector-overview" aria-labelledby="inspectorOverviewTitle">
        <header class="inspector-section-heading">
          <p class="section-kicker">先看结论</p>
          <h4 id="inspectorOverviewTitle">发生了什么</h4>
        </header>
        <dl class="summary-facts">
          <div><dt>当前情况</dt><dd>${escapeHtml(summary.happened)}</dd></div>
          <div><dt>主要风险</dt><dd>${escapeHtml(summary.risk)}</dd></div>
          <div><dt>你可以做什么</dt><dd>${escapeHtml(summary.next)}</dd></div>
        </dl>
      </section>
    `;
  }

  function renderReviewDetails(context) {
    if (!context) return "";
    return `
      <section class="inspector-section" id="proposalContextPanel" tabindex="-1">
        <header class="inspector-section-heading">
          <p class="section-kicker">建议详情</p>
          <h4>影响与来源</h4>
        </header>
        <dl class="summary-facts">
          <div><dt>影响对象</dt><dd>${escapeHtml(context.target)}</dd></div>
          <div><dt>风险</dt><dd>${escapeHtml(context.risk)}</dd></div>
          <div><dt>有效期</dt><dd>${escapeHtml(context.expires)}</dd></div>
        </dl>
        <p class="contract-gap-note">当前 ReviewItem 未投影 before、after、原因和影响摘要；这里是基于 AgentProposal 的 PROPOSED 静态目标。</p>
      </section>
    `;
  }

  function renderPermissionDetails(context) {
    if (!context) return "";
    return `
      <section class="inspector-section" id="permissionScopePanel" tabindex="-1">
        <header class="inspector-section-heading">
          <p class="section-kicker">访问范围</p>
          <h4>这次请求会做什么</h4>
        </header>
        <dl class="summary-facts">
          <div><dt>用途</dt><dd>${escapeHtml(context.purpose)}</dd></div>
          <div><dt>访问位置</dt><dd>${escapeHtml(context.target)}</dd></div>
          <div><dt>数据范围</dt><dd>${escapeHtml(context.dataScope)}</dd></div>
          <div><dt>外部传输</dt><dd>${escapeHtml(context.transmission)}</dd></div>
          <div><dt>有效期</dt><dd>${escapeHtml(context.duration)}</dd></div>
          <div><dt>撤销方式</dt><dd>${escapeHtml(context.revocation)}</dd></div>
        </dl>
        <p class="contract-gap-note">当前 ReviewItem 未投影完整 canonical_scope，也不能安全表达“仅允许本次”。</p>
        <details class="permission-technical">
          <summary>工具与策略字段</summary>
          <dl class="summary-facts">
            <div><dt>工具</dt><dd>${escapeHtml(context.tool)}</dd></div>
            <div><dt>能力</dt><dd><code>${escapeHtml(context.capability)}</code></dd></div>
            <div><dt>当前策略</dt><dd><code>${escapeHtml(context.currentPolicy)}</code></dd></div>
          </dl>
        </details>
      </section>
    `;
  }

  function renderBoundary(boundary) {
    const protectionResult =
      boundary.externalTransmission === "possible"
        ? "在传输边界确认前，相关外部动作保持关闭。"
        : boundary.externalTransmission === "sent"
          ? "已经发生外部传输，请查看来源与审计记录。"
          : "当前没有足够依据证明内容只在本地处理。";
    return `
      <section class="inspector-section" aria-labelledby="privacyBoundaryTitle">
        <header class="inspector-section-heading">
          <p class="section-kicker">模型与隐私</p>
          <h4 id="privacyBoundaryTitle">${escapeHtml(boundary.privacyLabel)}</h4>
        </header>
        <dl class="summary-facts">
          <div><dt>模型</dt><dd>${escapeHtml(boundary.modelLabel)}</dd></div>
          <div><dt>供应商</dt><dd>${escapeHtml(boundary.providerLabel)}</dd></div>
          <div><dt>保护结果</dt><dd>${escapeHtml(protectionResult)}</dd></div>
        </dl>
      </section>
    `;
  }

  function renderEvidenceRefs(evidenceRefs) {
    return `
      <section class="inspector-section" aria-labelledby="evidenceRefsTitle">
        <header class="inspector-section-heading">
          <p class="section-kicker">参考依据</p>
          <h4 id="evidenceRefsTitle">判断来自哪里</h4>
        </header>
        <div class="evidence-records">
          ${evidenceRefs
            .map(
              (item) => `
                <article class="evidence-record" data-evidence-id="${escapeHtml(item.id)}" tabindex="-1">
                  <strong>${escapeHtml(item.label)}</strong>
                  <p>${escapeHtml(sourceLabel(item.source))} · ${escapeHtml(sensitivityLabel(item.sensitivity))}</p>
                  <details class="evidence-technical">
                    <summary>技术标识</summary>
                    <code>${escapeHtml(item.id)}</code>
                  </details>
                </article>
              `,
            )
            .join("")}
        </div>
      </section>
    `;
  }

  function renderFieldSources(state) {
    const sources = collectFieldSources(state);
    return `
      <div class="source-map-list">
        ${sources
          .map(
            (item) => `
              <div class="source-map-row">
                <span>${escapeHtml(item.element)}</span>
                <code>${escapeHtml(item.sourceRef)}</code>
              </div>
            `,
          )
          .join("")}
      </div>
    `;
  }

  function renderDebugActions(state) {
    const actions = state.actions.debugOnly || [];
    return `
      <div
        class="debug-panel"
        id="debugPanel"
        aria-labelledby="debugPanelTitle"
        tabindex="-1"
      >
        <header class="inspector-section-heading">
          <p class="section-kicker">高级</p>
          <h4 id="debugPanelTitle">调试信息</h4>
        </header>
        <code class="fixture-id">${escapeHtml(state.fixtureId)}</code>
        <div class="debug-actions">
          ${actions.map((action) => actionButton(action, "debug")).join("")}
        </div>
      </div>
    `;
  }

  function renderTechnicalDetails(state) {
    const boundary = state.privacyBoundary;
    return `
      <section class="inspector-section technical-section">
        <details class="technical-details">
          <summary>技术详情与字段来源</summary>
          <div class="technical-details-body">
            <p class="technical-subheading">模型路由字段</p>
            <dl class="structured-fields">
              <div><dt>routeType</dt><dd>${escapeHtml(boundary.routeType)}</dd></div>
              <div><dt>externalTransmission</dt><dd>${escapeHtml(boundary.externalTransmission)}</dd></div>
              <div><dt>risk</dt><dd>${escapeHtml(boundary.risk)}</dd></div>
              <div><dt>blockedReason</dt><dd>${escapeHtml(boundary.blockedReason || "none")}</dd></div>
            </dl>
            <p class="technical-subheading">字段来源</p>
            ${renderFieldSources(state)}
            ${renderDebugActions(state)}
          </div>
        </details>
      </section>
    `;
  }

  function renderInspector(state) {
    const isFocusedWorkspace = state.inspectorMode === "on_demand";
    inspectorContent.innerHTML = `
      ${isFocusedWorkspace ? "" : renderInspectorOverview(state.inspectorSummary)}
      ${renderReviewDetails(state.reviewContext)}
      ${renderPermissionDetails(state.permissionContext)}
      ${isFocusedWorkspace ? "" : renderBoundary(state.privacyBoundary)}
      ${renderEvidenceRefs(state.evidenceRefs)}
      <section class="inspector-section" aria-labelledby="limitationsTitle">
        <header class="inspector-section-heading">
          <p class="section-kicker">当前限制</p>
          <h4 id="limitationsTitle">还不能证明什么</h4>
        </header>
        <ul class="limitation-list">
          ${state.limitations.map((item) => `<li>${escapeHtml(item)}</li>`).join("")}
        </ul>
      </section>
      ${renderTechnicalDetails(state)}
    `;
  }

  function findAction(state, id, lane) {
    if (!state) return null;
    const laneKey = lane === "debug" ? "debugOnly" : lane;
    return (state.actions[laneKey] || []).find((action) => action.id === id) || null;
  }

  function contractPayload(action, lane, state) {
    if (lane === "debug" && action.fixtureBehavior?.payload === "state") {
      return {
        fixtureId: state.fixtureId,
        envelope: state.envelope,
        privacyBoundary: state.privacyBoundary,
        actions: state.actions,
      };
    }
    const { fixtureBehavior, sourceRef, ...contractFields } = action;
    return { lane, contractFields, sourceRef };
  }

  function showActionDialog(action, lane, state, trigger) {
    const behavior = action.fixtureBehavior || {};
    actionDialogTitle.textContent = behavior.title || action.label;
    actionDialogSummary.textContent =
      behavior.message ||
      (lane === "debug"
        ? "仅展示静态 fixture 数据，不执行调试命令或导出。"
        : "这是可验证的静态反馈，不执行后端命令或持久写入。");
    actionDialogPayload.textContent = JSON.stringify(contractPayload(action, lane, state), null, 2);
    actionDialogPayload.hidden = lane !== "debug";
    if (behavior.type === "confirm_transition") {
      actionDialogActions.innerHTML = `
        <button class="action-button" type="button" data-dialog-cancel>取消</button>
        <button class="action-button is-primary" type="button" data-dialog-confirm>${escapeHtml(behavior.confirmLabel || "确认")}</button>
      `;
      actionDialogActions.querySelector("[data-dialog-cancel]").addEventListener("click", () => {
        actionDialog.close();
      });
      actionDialogActions.querySelector("[data-dialog-confirm]").addEventListener("click", () => {
        actionDialog.close();
        renderState(behavior.stateId, { announceChange: false });
        announce(`${action.label}：静态流程已进入“已批准，尚未应用”状态`);
      });
    } else {
      actionDialogActions.innerHTML = `
        <button class="action-button is-primary" type="button" data-dialog-close>关闭</button>
      `;
      actionDialogActions.querySelector("[data-dialog-close]").addEventListener("click", () => {
        actionDialog.close();
      });
    }
    actionDialog.showModal();
    announce(`已打开 ${action.label} 的静态结果`);
    lastInspectorTrigger = trigger;
  }

  function bindActionHandlers(state) {
    document.querySelectorAll("[data-action-id]").forEach((button) => {
      if (button.disabled) return;
      button.addEventListener("click", (event) => {
        event.preventDefault();
        const lane = button.getAttribute("data-action-lane");
        const action = findAction(state, button.getAttribute("data-action-id"), lane);
        if (!action) return;
        const behavior = action.fixtureBehavior;
        if (behavior?.type === "navigate") {
          renderState(behavior.stateId);
          announce(`${action.label}：已跳转到对应静态状态`);
          return;
        }
        if (behavior?.type === "open_inspector") {
          openInspector(button, behavior.evidenceId, behavior.sectionId);
          announce(`${action.label}：已打开依据与风险`);
          return;
        }
        showActionDialog(action, lane, state, button);
      });
    });
  }

  function highlightEvidence(evidenceId) {
    if (!evidenceId) return null;
    const target = Array.from(inspector.querySelectorAll("[data-evidence-id]")).find(
      (node) => node.getAttribute("data-evidence-id") === evidenceId,
    );
    if (!target) return null;
    target.classList.add("is-highlighted");
    target.scrollIntoView({ block: "nearest" });
    window.setTimeout(() => target.classList.remove("is-highlighted"), 1600);
    return target;
  }

  function openInspector(trigger, evidenceId = null, sectionId = null) {
    lastInspectorTrigger = trigger || document.activeElement;
    if (mobileQuery.matches) {
      inspector.classList.add("is-open");
      inspector.removeAttribute("role");
      if (!inspector.open) inspector.showModal();
      inspector.setAttribute("aria-hidden", "false");
      mobileEvidenceButton.setAttribute("aria-expanded", "true");
    } else if (workbenchShell.classList.contains("has-collapsible-inspector")) {
      inspector.setAttribute("open", "");
      inspector.setAttribute("role", "complementary");
      inspector.setAttribute("aria-hidden", "false");
      workbenchShell.classList.add("is-inspector-open");
    }
    const section = sectionId ? document.getElementById(sectionId) : null;
    if (section) {
      const parentDetails = section.closest("details");
      if (parentDetails) parentDetails.open = true;
      section.classList.add("is-highlighted");
      window.setTimeout(() => section.classList.remove("is-highlighted"), 1600);
    }
    const target = highlightEvidence(evidenceId) || section || inspector;
    if (target !== inspector) target.scrollIntoView({ block: "nearest" });
    if (!mobileQuery.matches && section && trigger instanceof HTMLElement) {
      trigger.focus({ preventScroll: true });
      return;
    }
    const focusTarget =
      target === inspector
        ? mobileQuery.matches
          ? inspectorCloseButton
          : inspector.querySelector("[data-evidence-id], summary, button:not(.inspector-close)") ||
            inspector
        : target.querySelector?.("button:not([disabled]), summary") || target;
    focusTarget.focus({ preventScroll: true });
    window.setTimeout(() => focusTarget.focus({ preventScroll: true }), 0);
  }

  function closeInspector({ restoreFocus = true } = {}) {
    if (mobileQuery.matches) {
      if (inspector.open) inspector.close();
      inspector.classList.remove("is-open");
      inspector.setAttribute("aria-hidden", "true");
      mobileEvidenceButton.setAttribute("aria-expanded", "false");
    } else {
      if (!workbenchShell.classList.contains("has-collapsible-inspector")) return;
      inspector.removeAttribute("open");
      inspector.setAttribute("aria-hidden", "true");
      workbenchShell.classList.remove("is-inspector-open");
    }
    if (restoreFocus && lastInspectorTrigger instanceof HTMLElement) {
      lastInspectorTrigger.focus();
    }
  }

  function openMobileNav(trigger) {
    lastDrawerTrigger = trigger;
    mobileNavOverlay.hidden = false;
    mobileMenuButton.setAttribute("aria-expanded", "true");
    window.requestAnimationFrame(() => mobileNavCloseButton.focus());
  }

  function closeMobileNav({ restoreFocus = true } = {}) {
    mobileNavOverlay.hidden = true;
    mobileMenuButton.setAttribute("aria-expanded", "false");
    if (restoreFocus && lastDrawerTrigger instanceof HTMLElement) lastDrawerTrigger.focus();
  }

  function trapFocus(container, event) {
    if (event.key !== "Tab") return;
    const focusable = Array.from(
      container.querySelectorAll('button:not([disabled]), select:not([disabled]), summary, [tabindex="0"]'),
    ).filter((node) => !node.hidden && node.offsetParent !== null);
    if (!focusable.length) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  function syncResponsiveA11y() {
    const state = stateById.get(currentStateId);
    const collapsible = state?.inspectorMode === "on_demand";
    workbenchShell.classList.toggle("has-collapsible-inspector", collapsible);
    workbenchShell.classList.remove("is-inspector-open");
    if (mobileQuery.matches) {
      if (inspector.open) inspector.close();
      inspector.classList.remove("is-open");
      inspector.removeAttribute("role");
      inspector.setAttribute("aria-hidden", "true");
      mobileEvidenceButton.setAttribute("aria-expanded", "false");
      return;
    }
    if (inspector.open) inspector.close();
    if (collapsible) {
      inspector.removeAttribute("open");
    } else {
      inspector.setAttribute("open", "");
    }
    inspector.classList.remove("is-open");
    inspector.setAttribute("role", "complementary");
    inspector.setAttribute("aria-hidden", collapsible ? "true" : "false");
    mobileEvidenceButton.setAttribute("aria-expanded", "false");
    if (!mobileNavOverlay.hidden) closeMobileNav({ restoreFocus: false });
  }

  function renderState(id, { announceChange = true } = {}) {
    const state = stateById.get(id) || states[0];
    if (!state) return;
    currentStateId = state.id;
    lastFixtureStateId = state.id;
    currentNavKey = state.navKey;
    surfaceTitle.textContent = state.surface.title;
    surfaceEyebrow.textContent = state.surface.eyebrow;
    inspectorTitle.textContent = state.inspectorHeading || "依据与风险";
    topPrimaryStatus.innerHTML = statusLabel(state.primaryStatus);
    renderPrivacyBoundary(state.privacyBoundary);
    renderQaSelector();
    renderWorkSurface(state);
    renderInspector(state);
    syncResponsiveA11y();
    renderNavigation();
    bindActionHandlers(state);
    renderStaticIcons();
    closeMobileNav({ restoreFocus: false });
    if (announceChange) announce(`已切换到 ${state.shortLabel}`);
  }

  function renderUnavailable(item) {
    currentStateId = null;
    currentNavKey = item.key;
    const unknown = unavailablePrivacyBoundary();
    workSurface.classList.remove("has-sticky-decisions");
    workSurface.classList.remove("is-workspace-focus");
    inspectorTitle.textContent = "依据与风险";
    surfaceTitle.textContent = item.label;
    surfaceEyebrow.textContent = "尚未开放";
    topPrimaryStatus.innerHTML = statusLabel({ label: "尚未开放", tone: "is-muted" });
    renderPrivacyBoundary(unknown);
    renderQaSelector(item.unavailable.title);
    workSurface.innerHTML = `
      <section class="unavailable-surface" role="status" aria-labelledby="unavailableTitle">
        <p class="section-kicker">未覆盖页面</p>
        <h3 id="unavailableTitle">${escapeHtml(item.unavailable.title)}</h3>
        <p>${escapeHtml(item.unavailable.detail)}</p>
        <button class="action-button is-primary" id="returnToTodayButton" type="button">返回今日</button>
      </section>
    `;
    inspectorContent.innerHTML = `
      <section class="inspector-section">
        <header class="inspector-section-heading">
          <p class="section-kicker">当前状态</p>
          <h4>没有可用依据</h4>
        </header>
        <p class="inspector-copy">任务独立入口尚未建立可信读模型，因此不会展示推断出的任务状态。</p>
      </section>
    `;
    syncResponsiveA11y();
    renderNavigation();
    renderStaticIcons();
    document.getElementById("returnToTodayButton").addEventListener("click", () => {
      renderState(states[0].id);
    });
    closeMobileNav({ restoreFocus: false });
    announce(`${item.label} 尚未开放，已显示明确反馈`);
  }

  function handleNavigation(item, trigger) {
    if (item.defaultStateId) {
      renderState(item.defaultStateId);
      return;
    }
    if (item.unavailable) renderUnavailable(item);
  }

  function openSupportDetails(trigger) {
    if (!currentStateId && lastFixtureStateId) {
      renderState(lastFixtureStateId, { announceChange: false });
    }
    const inspectorTrigger =
      mobileQuery.matches && mobileNavDrawer.contains(trigger) ? mobileMenuButton : trigger;
    closeMobileNav({ restoreFocus: false });
    openInspector(inspectorTrigger, null, "debugPanel");
    announce("已打开支持信息");
  }

  qaStateSelector.addEventListener("change", () => renderState(qaStateSelector.value));
  privacyStatusButton.addEventListener("click", (event) => {
    if (mobileQuery.matches) event.preventDefault();
    openInspector(privacyStatusButton);
  });
  mobileEvidenceButton.addEventListener("click", (event) => {
    event.preventDefault();
    openInspector(mobileEvidenceButton);
  });
  inspectorCloseButton.addEventListener("click", () => closeInspector());
  supportDetailsButton.addEventListener("click", (event) => {
    if (mobileQuery.matches) event.preventDefault();
    openSupportDetails(supportDetailsButton);
  });
  mobileSupportDetailsButton.addEventListener("click", (event) => {
    event.preventDefault();
    openSupportDetails(mobileSupportDetailsButton);
  });
  mobileMenuButton.addEventListener("click", () => openMobileNav(mobileMenuButton));
  mobileNavCloseButton.addEventListener("click", () => closeMobileNav());
  mobileNavScrim.addEventListener("click", () => closeMobileNav());

  mobileNavDrawer.addEventListener("keydown", (event) => trapFocus(mobileNavDrawer, event));
  inspector.addEventListener("cancel", (event) => {
    event.preventDefault();
    closeInspector();
  });
  inspector.addEventListener("click", (event) => {
    if (!mobileQuery.matches || event.target !== inspector) return;
    const rect = inspector.getBoundingClientRect();
    const outside =
      event.clientX < rect.left ||
      event.clientX > rect.right ||
      event.clientY < rect.top ||
      event.clientY > rect.bottom;
    if (outside) closeInspector();
  });
  document.addEventListener("keydown", (event) => {
    if (event.key !== "Escape") return;
    if (!mobileNavOverlay.hidden) {
      closeMobileNav();
      return;
    }
    if (
      (mobileQuery.matches && inspector.open) ||
      (!mobileQuery.matches && workbenchShell.classList.contains("is-inspector-open"))
    ) {
      closeInspector();
    }
  });
  mobileQuery.addEventListener("change", syncResponsiveA11y);

  renderStaticIcons();
  renderState(currentStateId, { announceChange: false });
  syncResponsiveA11y();
});
