(function () {
  "use strict";

  const model = window.OPENLIFE_BLUEPRINT_DATA;
  if (!model) throw new Error("OPENLIFE_BLUEPRINT_DATA is required");

  const $ = (selector, root = document) => root.querySelector(selector);
  const $$ = (selector, root = document) => Array.from(root.querySelectorAll(selector));
  const escapeHtml = value =>
    String(value ?? "")
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll('"', "&quot;")
      .replaceAll("'", "&#039;");

  const nodes = {
    select: $("#blueprintSelect"),
    shell: $("#workbenchShell"),
    workSurface: $("#workSurface"),
    contextEyebrow: $("#contextEyebrow"),
    contextTitle: $("#contextTitle"),
    primaryStatus: $("#primaryStatus"),
    sidebarNav: $("#sidebarNav"),
    utilityNav: $("#utilityNav"),
    mobileBottomNav: $("#mobileBottomNav"),
    mobileDrawer: $("#mobileDrawer"),
    mobileDrawerNav: $("#mobileDrawerNav"),
    mobileUtilityNav: $("#mobileUtilityNav"),
    drawerPrivacyBoundary: $("#drawerPrivacyBoundary"),
    openMobileMenu: $("#openMobileMenu"),
    closeMobileMenu: $("#closeMobileMenu"),
    privacyBoundaryButton: $("#privacyBoundaryButton"),
    sidebarPrivacyTitle: $("#sidebarPrivacyTitle"),
    sidebarPrivacyMeta: $("#sidebarPrivacyMeta"),
    openInspector: $("#openInspector"),
    closeInspector: $("#closeInspector"),
    inspector: $("#evidenceInspector"),
    inspectorBody: $("#inspectorBody"),
    inspectorBackdrop: $("#inspectorBackdrop"),
    dialog: $("#feedbackDialog"),
    dialogKicker: $("#dialogKicker"),
    dialogTitle: $("#dialogTitle"),
    dialogBody: $("#dialogBody"),
    dialogIcon: $("#dialogIcon"),
    dialogActions: $("#dialogActions"),
    toast: $("#toast"),
    liveRegion: $("#liveRegion"),
  };

  let currentScreenKey = model.defaultScreen;
  let inspectorOpen = false;
  let inspectorTrigger = null;
  let activeTaskFilter = "全部";
  let selectedTaskId = "task:hangzhou-weekend";
  let toastTimer = null;
  let permissionFlowStage = "idle";
  let permissionFlowToken = 0;
  let reviewFixtureState = "pending";
  let reviewEditedAfter = "";
  let settingsCategory = "models";
  let settingsSearchQuery = "";
  let settingsState = "clean";
  let settingsDraft = { ...model.screens.settings.config };
  let fixtureAttachments = [];
  const detachedResourceIds = new Set();
  let dialogTrigger = null;

  const iconPaths = {
    calendar: '<rect x="4" y="5" width="16" height="15" rx="2"/><path d="M8 3v4M16 3v4M4 10h16"/>',
    workspace: '<rect x="3" y="4" width="18" height="13" rx="2"/><path d="M8 21h8M12 17v4"/>',
    tasks: '<path d="m4 6 2 2 4-4M11 6h9M4 13l2 2 4-4M11 13h9M4 20l2 2 4-4M11 20h9"/>',
    review:
      '<path d="M12 3 4.5 6v5.5c0 4.6 3.2 7.8 7.5 9.5 4.3-1.7 7.5-4.9 7.5-9.5V6L12 3Z"/><path d="m9 12 2 2 4-4"/>',
    lifemodel:
      '<circle cx="12" cy="8" r="3.5"/><path d="M5 21c.7-4.1 3.1-6.2 7-6.2s6.3 2.1 7 6.2"/>',
    settings:
      '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-2.8 2.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6v.2h-4V21a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1L4.2 17l.1-.1a1.7 1.7 0 0 0 .3-1.9A1.7 1.7 0 0 0 3 14H2.8v-4H3a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9L4.2 7 7 4.2l.1.1a1.7 1.7 0 0 0 1.9.3A1.7 1.7 0 0 0 10 3v-.2h4V3a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1L19.8 7l-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.6 1h.2v4H21a1.7 1.7 0 0 0-1.6 1Z"/>',
    terminal: '<path d="m5 7 4 5-4 5M12 17h7"/>',
    shield: '<path d="M12 3 4.5 6v5.5c0 4.6 3.2 7.8 7.5 9.5 4.3-1.7 7.5-4.9 7.5-9.5V6L12 3Z"/>',
    history: '<path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5M12 7v5l3 2"/>',
    evidence:
      '<path d="M3 12s3.5-6 9-6 9 6 9 6-3.5 6-9 6-9-6-9-6Z"/><circle cx="12" cy="12" r="2.5"/>',
    arrow: '<path d="M5 12h14M14 7l5 5-5 5"/>',
    check: '<path d="m5 12 4 4L19 6"/>',
    lock: '<rect x="4" y="10" width="16" height="11" rx="2"/><path d="M8 10V7a4 4 0 0 1 8 0v3"/>',
  };

  function icon(name, className = "") {
    return `<svg class="${escapeHtml(className)}" viewBox="0 0 24 24" aria-hidden="true">${iconPaths[name] || iconPaths.evidence}</svg>`;
  }

  function announce(message) {
    nodes.liveRegion.textContent = "";
    window.setTimeout(() => {
      nodes.liveRegion.textContent = message;
    }, 20);
  }

  function screen() {
    return model.screens[currentScreenKey];
  }

  function navButton(item, context) {
    const active = item.key === screen().routeKey;
    const badge = item.badge ? `<span class="nav-badge">${escapeHtml(item.badge)}</span>` : "";
    const description =
      context === "bottom" ? "" : `<small>${escapeHtml(item.description)}</small>`;
    return `
      <button
        class="nav-item ${active ? "is-active" : ""} ${context === "bottom" ? "is-bottom" : ""}"
        type="button"
        data-nav-key="${escapeHtml(item.key)}"
        ${active ? 'aria-current="page"' : ""}
      >
        <span class="nav-icon">${icon(item.icon)}</span>
        <span class="nav-copy"><strong>${escapeHtml(item.label)}</strong>${description}</span>
        ${badge}
      </button>
    `;
  }

  function renderNavigation() {
    const primary = model.navigation.filter(item => item.placement === "primary");
    const utility = model.navigation.filter(item => item.placement === "utility");
    const settingsMode = screen().routeKey === "settings";
    nodes.shell.classList.toggle("is-settings-mode", settingsMode);
    if (settingsMode) {
      const categories = screen().categories || [];
      nodes.sidebarNav.innerHTML = `
        <button class="settings-back" type="button" data-settings-back>
          ${icon("arrow")}<span>返回工作台</span>
        </button>
        <label class="settings-search">
          <span class="sr-only">搜索设置</span>
          <input type="search" value="${escapeHtml(settingsSearchQuery)}" placeholder="搜索设置" data-settings-search />
        </label>
        <p class="settings-search-status" data-settings-search-status aria-live="polite"></p>
        <div class="settings-category-list">
          ${categories
            .map(
              item => `
                <button
                  type="button"
                  class="settings-category ${item.id === settingsCategory ? "is-active" : ""}"
                  data-settings-category="${escapeHtml(item.id)}"
                  data-settings-keywords="${escapeHtml(`${item.label} ${item.keywords}`.toLowerCase())}"
                  ${item.id === settingsCategory ? 'aria-current="page"' : ""}
                >${escapeHtml(item.label)}</button>
              `
            )
            .join("")}
        </div>
      `;
      nodes.utilityNav.innerHTML = `
        <div class="settings-sidebar-note">
          <strong>边界由后端确认</strong>
          <span>配置、测试和实际路线分开显示</span>
        </div>
      `;
    } else {
      nodes.sidebarNav.innerHTML = primary.map(item => navButton(item, "sidebar")).join("");
      nodes.utilityNav.innerHTML = utility.map(item => navButton(item, "sidebar")).join("");
    }
    nodes.mobileDrawerNav.innerHTML = primary.map(item => navButton(item, "drawer")).join("");
    nodes.mobileUtilityNav.innerHTML = utility.map(item => navButton(item, "drawer")).join("");
    nodes.mobileBottomNav.innerHTML = primary
      .filter(item => item.mobile)
      .map(item => navButton(item, "bottom"))
      .join("");

    $$("[data-nav-key]").forEach(button => {
      button.addEventListener("click", () => {
        const item = model.navigation.find(
          candidate => candidate.key === button.getAttribute("data-nav-key")
        );
        if (!item) return;
        if (nodes.mobileDrawer.open) nodes.mobileDrawer.close();
        if (item.outcome === "inspector" || !item.screen) {
          openInspector(button, true);
          return;
        }
        setScreen(item.screen, { focus: true, announce: true });
      });
    });

    $("[data-settings-back]", nodes.sidebarNav)?.addEventListener("click", () =>
      setScreen("today-ready", { focus: true, announce: true })
    );
    $$("[data-settings-category]", nodes.sidebarNav).forEach(button => {
      button.addEventListener("click", () => {
        settingsCategory = button.getAttribute("data-settings-category") || "models";
        renderNavigation();
        renderWorkSurface();
        nodes.workSurface.focus({ preventScroll: true });
        announce(`设置分类已切换为${button.textContent.trim()}`);
      });
    });
    const search = $("[data-settings-search]", nodes.sidebarNav);
    search?.addEventListener("input", () => {
      settingsSearchQuery = search.value;
      applySettingsSearch();
    });
    applySettingsSearch();
  }

  function applySettingsSearch() {
    const query = settingsSearchQuery.trim().toLowerCase();
    const categories = $$("[data-settings-category]", nodes.sidebarNav);
    let visible = 0;
    categories.forEach(button => {
      const match = !query || button.getAttribute("data-settings-keywords").includes(query);
      button.hidden = !match;
      if (match) visible += 1;
    });
    const status = $("[data-settings-search-status]", nodes.sidebarNav);
    if (status) status.textContent = query ? `找到 ${visible} 个分类` : "";
  }

  function renderStatus(status) {
    nodes.primaryStatus.className = `primary-status is-${status.tone}`;
    nodes.primaryStatus.innerHTML = `<span aria-hidden="true"></span>${escapeHtml(status.label)}`;
  }

  function renderPrivacy(privacy) {
    nodes.sidebarPrivacyTitle.textContent = privacy.title;
    nodes.sidebarPrivacyMeta.textContent = privacy.meta;
    const sidebarDot = $(".status-dot", nodes.privacyBoundaryButton);
    if (sidebarDot) sidebarDot.className = `status-dot is-${privacy.tone || "warning"}`;
    nodes.drawerPrivacyBoundary.innerHTML = `
      <span class="status-dot is-${escapeHtml(privacy.tone || "warning")}" aria-hidden="true"></span>
      <div><strong>${escapeHtml(privacy.title)}</strong><small>${escapeHtml(privacy.meta)}</small></div>
    `;
  }

  function actionButton(item, options = {}) {
    const reasonId = `reason-${item.id.replace(/[^a-zA-Z0-9_-]/g, "-")}`;
    const disabled = item.enabled ? "" : "disabled";
    const described = item.enabled ? "" : `aria-describedby="${reasonId}"`;
    const tone =
      item.kind === "approve"
        ? "is-primary"
        : item.kind === "reject"
          ? "is-danger-quiet"
          : item.kind === "apply"
            ? "is-primary"
            : "is-secondary";
    return `
      <div class="action-control ${options.compact ? "is-compact" : ""}">
        <button
          class="action-button ${tone}"
          type="button"
          data-action-id="${escapeHtml(item.id)}"
          ${disabled}
          ${described}
        >${escapeHtml(item.label)}</button>
        ${
          item.enabled || options.hideReason
            ? ""
            : `<span class="disabled-reason" id="${reasonId}">${escapeHtml(item.disabledReason)}</span>`
        }
      </div>
    `;
  }

  function actionRow(actions, className = "") {
    if (!actions?.length) return "";
    return `<div class="action-row ${escapeHtml(className)}">${actions.map(item => actionButton(item)).join("")}</div>`;
  }

  function renderPageIntro(data) {
    return "";
  }

  function renderToday(data) {
    return `
      <div class="surface-inner today-surface">
        ${renderPageIntro(data)}
        <section class="focus-hero" aria-labelledby="todayFocusTitle">
          <div class="focus-copy">
            <span class="section-label">${escapeHtml(data.focus.kicker)}</span>
            <h2 id="todayFocusTitle">${escapeHtml(data.focus.title)}</h2>
            <p>${escapeHtml(data.focus.summary)}</p>
          </div>
          <dl class="fact-strip">
            ${data.facts
              .map(
                fact => `
                  <div>
                    <dt>${escapeHtml(fact.label)}</dt>
                    <dd>${escapeHtml(fact.value)}</dd>
                  </div>
                `
              )
              .join("")}
          </dl>
        </section>

        <section class="attention-strip is-${escapeHtml(data.attention.tone)}" aria-labelledby="attentionTitle">
          <div>
            <span class="section-label">${escapeHtml(data.attention.kicker)}</span>
            <h3 id="attentionTitle">${escapeHtml(data.attention.title)}</h3>
          </div>
          <p>${escapeHtml(data.attention.body)}</p>
        </section>

        <section class="next-action-band" aria-labelledby="todayNextTitle">
          <div>
            <span class="section-label">下一步</span>
            <h3 id="todayNextTitle">选择下一步</h3>
          </div>
          ${actionRow(data.actions, "is-right")}
        </section>

        <div class="today-columns">
          <section class="content-section" aria-labelledby="scheduleTitle">
            <header class="section-heading">
              <div><span class="section-label">今日安排</span><h3 id="scheduleTitle">接下来怎么推进</h3></div>
            </header>
            <div class="schedule-list">
              ${data.schedule
                .map(
                  item => `
                    <article class="schedule-row is-${escapeHtml(item.state)}">
                      <time>${escapeHtml(item.time)}</time>
                      <div><h4>${escapeHtml(item.title)}</h4><p>${escapeHtml(item.body)}</p></div>
                      <span>${escapeHtml(item.meta)}</span>
                    </article>
                  `
                )
                .join("")}
            </div>
          </section>

          <section class="content-section boundary-section" aria-labelledby="boundaryTitle">
            <header class="section-heading">
              <div><span class="section-label">当前边界</span><h3 id="boundaryTitle">今天不会自动发生的事</h3></div>
            </header>
            <div class="boundary-list">
              ${data.boundaries
                .map(
                  item => `
                    <article class="boundary-row">
                      <span>${icon(item.icon)}</span>
                      <div><h4>${escapeHtml(item.title)}</h4><p>${escapeHtml(item.body)}</p></div>
                    </article>
                  `
                )
                .join("")}
            </div>
          </section>
        </div>
      </div>
    `;
  }

  function renderPermissionEvent(data, event) {
    const permissionActions = data.actions.filter(item => item.lane === "review");
    const isBusy = ["recording", "refreshing", "resuming"].includes(permissionFlowStage);
    const stageCopy = {
      recording: "正在记录一次性决定…",
      refreshing: "决定已返回，正在刷新审核与任务状态…",
      resuming: "范围已核对，正在请求恢复当前任务…",
      rejected: "你已拒绝本次访问。任务保持暂停，没有读取文件。",
    }[permissionFlowStage];
    return `
      <article class="timeline-event-card" aria-labelledby="permissionEventTitle">
        <header>
          <div><span class="section-label">${escapeHtml(event.label)}</span><h3 id="permissionEventTitle">${escapeHtml(event.title)}</h3></div>
          <span>${escapeHtml(event.meta)}</span>
        </header>
        <p>${escapeHtml(event.body)}</p>
        <ul class="scope-summary" aria-label="访问范围摘要">
          <li>${escapeHtml(data.permission.dataScope)}</li>
          <li>${escapeHtml(data.permission.duration)}</li>
          <li class="${data.privacy.externalTransmission === "unknown" ? "is-warning" : ""}">${escapeHtml(data.permission.transmission)}</li>
        </ul>
        <p class="fail-closed-line">${escapeHtml(
          data.actions.find(item => item.id === "workspace:allow-once")?.enabled
            ? "批准后先刷新审核与任务状态；只有同一动作仍可恢复时才继续。"
            : "范围或传输边界不完整时保持暂停，不从文件夹名称推断权限。"
        )}</p>
        ${stageCopy ? `<p class="permission-flow-state" role="status">${escapeHtml(stageCopy)}</p>` : ""}
        <div class="decision-inline">
          <div><span class="section-label">下一步</span><h4>决定这次访问</h4></div>
          <div>
            <div class="action-row is-review">${permissionActions
              .map(item =>
                actionButton(
                  isBusy ? { ...item, enabled: false, disabledReason: "静态状态转换进行中" } : item,
                  { hideReason: false, compact: true }
                )
              )
              .join("")}</div>
          </div>
        </div>
      </article>
    `;
  }

  function workspaceResources(data) {
    const base = (data.resources || []).filter(item => !detachedResourceIds.has(item.id));
    return [...base, ...fixtureAttachments];
  }

  function renderResourceTray(data) {
    const resources = workspaceResources(data);
    if (!resources.length) return "";
    return `
      <div class="resource-tray" aria-label="当前任务附件">
        ${resources
          .map(
            item => `
              <div class="resource-chip is-${escapeHtml(item.state)}">
                <span>${icon("evidence")}</span>
                <span><strong>${escapeHtml(item.name)}</strong><small>${escapeHtml(item.meta)}</small></span>
                <button
                  type="button"
                  class="resource-remove"
                  data-resource-remove="${escapeHtml(item.id)}"
                  aria-label="移除 ${escapeHtml(item.name)}"
                  ${item.state === "importing" ? "disabled" : ""}
                >×</button>
              </div>
            `
          )
          .join("")}
      </div>
    `;
  }

  function renderWorkspace(data) {
    const primaryAction = data.actions.find(item => item.id === "workspace:view-task-evidence");
    return `
      <div class="surface-inner workspace-surface">
        ${renderPageIntro(data)}
        <section class="workspace-task-head" aria-labelledby="workspaceTaskTitle">
          <div>
            <span class="section-label">${escapeHtml(data.task.kicker)}</span>
            <h2 id="workspaceTaskTitle">${escapeHtml(data.task.title)}</h2>
            <p>${escapeHtml(data.task.summary)}</p>
          </div>
          ${actionButton(primaryAction, { compact: true })}
        </section>

        <section class="workspace-timeline" aria-labelledby="timelineTitle">
          <header class="section-heading workspace-timeline-heading">
            <div><span class="section-label">执行记录</span><h3 id="timelineTitle">当前任务进度</h3></div>
            <p>只展开当前需要处理的事件</p>
          </header>
          <ol>
            ${data.timeline
              .map(event => {
                const marker =
                  event.state === "done" ? icon("check") : event.state === "waiting" ? "!" : "";
                return `
                  <li class="timeline-event is-${escapeHtml(event.state)}">
                    <span class="timeline-marker" aria-hidden="true">${marker}</span>
                    <div class="timeline-event-content">
                      ${
                        event.state === "waiting"
                          ? renderPermissionEvent(data, event)
                          : `<div class="timeline-compact"><div><span class="section-label">${escapeHtml(event.label)}</span><h3>${escapeHtml(event.title)}</h3><p>${escapeHtml(event.body)}</p>${event.sources?.length ? `<ul class="timeline-sources">${event.sources.map(source => `<li>${escapeHtml(source)}</li>`).join("")}</ul>` : ""}</div><span>${escapeHtml(event.meta)}</span></div>`
                      }
                    </div>
                  </li>
                `;
              })
              .join("")}
          </ol>
        </section>

        <section class="workspace-composer" aria-label="补充任务说明">
          <button class="icon-button" type="button" data-static-feedback="attach" aria-label="添加上下文" title="添加上下文">
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 5v14M5 12h14"/></svg>
          </button>
          <label class="sr-only" for="workspaceComposer">补充任务说明</label>
          <textarea id="workspaceComposer" rows="1" placeholder="补充说明，或纠正 OpenLife 的理解…"></textarea>
          <button class="composer-send" type="button" data-static-feedback="composer">发送补充</button>
        </section>
        ${renderResourceTray(data)}
      </div>
    `;
  }

  function taskStateIcon(state) {
    if (state === "done") return icon("check");
    if (state === "waiting") return "!";
    return '<span class="pulse-dot"></span>';
  }

  function renderTasks(data) {
    const stateMap = {
      全部: () => true,
      进行中: item => item.state === "running",
      需要我: item => item.state === "waiting",
      已完成: item => item.state === "done",
    };
    const visibleTasks = data.taskItems.filter(stateMap[activeTaskFilter] || stateMap["全部"]);
    const selected = data.taskItems.find(item => item.id === selectedTaskId) || data.taskItems[0];
    return `
      <div class="tasks-surface">
        ${renderPageIntro(data)}
        <header class="task-toolbar">
          <div class="segmented-control" role="group" aria-label="任务状态筛选">
            ${data.filters
              .map(
                filter =>
                  `<button type="button" data-task-filter="${escapeHtml(filter)}" class="${filter === activeTaskFilter ? "is-active" : ""}">${escapeHtml(filter)}</button>`
              )
              .join("")}
          </div>
          <button class="icon-button" type="button" data-action-id="tasks:inspect" aria-label="查看任务依据" title="查看任务依据">${icon("evidence")}</button>
        </header>
        <div class="tasks-split">
          <section class="task-list-pane" aria-labelledby="taskListTitle">
            <header><span class="section-label">任务列表</span><h2 id="taskListTitle">最近工作</h2></header>
            <div class="task-list">
              ${
                visibleTasks
                  .map(
                    item => `
                    <button type="button" class="task-list-item ${item.id === selected.id ? "is-selected" : ""}" data-task-id="${escapeHtml(item.id)}">
                      <span class="task-state-icon is-${escapeHtml(item.state)}">${taskStateIcon(item.state)}</span>
                      <span class="task-list-copy">
                        <span><strong>${escapeHtml(item.title)}</strong><small>${escapeHtml(item.updated)}</small></span>
                        <em>${escapeHtml(item.stateLabel)}</em>
                        <p>${escapeHtml(item.summary)}</p>
                      </span>
                    </button>
                  `
                  )
                  .join("") || '<p class="empty-list">当前筛选下没有任务。</p>'
              }
            </div>
          </section>
          <section class="task-detail-pane" aria-labelledby="taskDetailTitle">
            <header class="task-detail-head">
              <div><span class="section-label">当前选择</span><h2 id="taskDetailTitle">${escapeHtml(selected.title)}</h2></div>
              <span class="status-lozenge is-${escapeHtml(selected.state)}">${escapeHtml(selected.stateLabel)}</span>
            </header>
            <p class="task-objective">${escapeHtml(selected.summary)}</p>
            <div class="task-detail-block">
              <span class="section-label">下一步</span>
              <h3>${escapeHtml(selected.next)}</h3>
              <p>${selected.state === "waiting" ? "任务会保持暂停，直到访问范围得到可执行的决定。" : selected.state === "running" ? "当前整理可以继续，外部发送仍需确认。" : "结果可查看，但不会自动创建新的长期状态。"}</p>
            </div>
            <ol class="mini-timeline">
              ${data.selectedTask.events
                .map(
                  (event, index) =>
                    `<li class="${index === 2 && selected.state === "waiting" ? "is-current" : ""}"><span>${index < 2 ? icon("check") : ""}</span><p>${escapeHtml(event)}</p></li>`
                )
                .join("")}
            </ol>
            ${actionRow(data.actions, "task-detail-actions")}
          </section>
        </div>
      </div>
    `;
  }

  function renderReviewQueue(data) {
    return `
      <aside class="review-queue" aria-labelledby="reviewQueueTitle">
        <header><span class="section-label">待处理</span><h2 id="reviewQueueTitle">建议与权限</h2></header>
        <div>
          ${data.queue
            .map(
              (item, index) => `
                <button type="button" class="review-queue-item ${index === 0 ? "is-selected" : ""}" data-review-queue-id="${escapeHtml(item.id)}">
                  <span>${escapeHtml(item.type)}</span>
                  <strong>${escapeHtml(item.title)}</strong>
                  <small>${escapeHtml(item.meta)}</small>
                </button>
              `
            )
            .join("")}
        </div>
      </aside>
    `;
  }

  function renderReview(data) {
    const reviewActions = data.actions.filter(item => item.lane === "review");
    const inspectAction = data.actions.find(item => item.kind === "inspect");
    const proposal = {
      ...data.proposal,
      after: reviewEditedAfter || data.proposal.after,
    };
    const decisionResult = {
      rejected: {
        title: "你已拒绝这项建议",
        body: "静态演示保持当前 LifeModel 不变；建议不会被应用。",
      },
      postponed: {
        title: "已设为稍后处理",
        body: "静态演示把建议保留为未批准状态，不会提前进入长期理解。",
      },
    }[reviewFixtureState];
    return `
      <div class="review-surface">
        ${renderPageIntro(data)}
        ${renderReviewQueue(data)}
        <article class="review-detail" aria-labelledby="proposalTitle">
          ${
            decisionResult
              ? `<section class="review-result-banner" role="status"><span class="section-label">静态决定结果</span><h2>${escapeHtml(decisionResult.title)}</h2><p>${escapeHtml(decisionResult.body)}</p><small>使用壳外 QA 场景选择器可重新载入待决定 fixture。</small></section>`
              : reviewEditedAfter
                ? '<p class="review-edited-note" role="status">建议内容已在静态演示中修改，状态仍为等待决定。</p>'
                : ""
          }
          <header class="proposal-head">
            <div>
              <span class="section-label">${escapeHtml(proposal.kicker)}</span>
              <h2 id="proposalTitle">${escapeHtml(proposal.title)}</h2>
              <p>${escapeHtml(proposal.summary)}</p>
            </div>
            ${actionButton(inspectAction, { compact: true })}
          </header>

          <section class="change-diff" aria-label="当前内容与建议内容对比">
            <div class="is-before"><span>当前</span><strong>${escapeHtml(proposal.before)}</strong></div>
            <span class="diff-arrow">${icon("arrow")}</span>
            <div class="is-after"><span>建议</span><strong>${escapeHtml(proposal.after)}</strong></div>
          </section>

          <dl class="proposal-facts">
            <div><dt>原因</dt><dd>${escapeHtml(proposal.reason)}</dd></div>
            <div><dt>来源</dt><dd>${escapeHtml(proposal.source)}</dd></div>
            <div><dt>风险</dt><dd>${escapeHtml(proposal.risk)}</dd></div>
            <div><dt>影响</dt><dd>${escapeHtml(proposal.impact)}</dd></div>
            <div><dt>有效期</dt><dd>${escapeHtml(proposal.expires)}</dd></div>
          </dl>

          <section class="impact-note">
            <span class="section-label">影响范围</span>
            <h3>只影响未来计划建议</h3>
            <p>不会自动移动已有日程；批准后也必须等待可验证的应用结果。</p>
          </section>

          ${
            decisionResult
              ? ""
              : `<footer class="review-decision-bar">
                  <div><span class="section-label">你的决定</span><h3>选择怎样处理这项建议</h3></div>
                  <div class="action-row is-review">${reviewActions.map(item => actionButton(item, { hideReason: true, compact: true })).join("")}</div>
                </footer>`
          }
        </article>
      </div>
    `;
  }

  function renderReviewApproved(data) {
    const applyAction = data.actions.find(item => item.kind === "apply");
    const otherActions = data.actions.filter(item => item.kind !== "apply");
    return `
      <div class="surface-inner approved-surface">
        ${renderPageIntro(data)}
        <section class="approved-result" aria-labelledby="approvedTitle">
          <span class="result-icon">${icon("check")}</span>
          <div>
            <span class="section-label">${escapeHtml(data.result.kicker)}</span>
            <h2 id="approvedTitle">${escapeHtml(data.result.title)}</h2>
            <p>${escapeHtml(data.result.summary)}</p>
          </div>
        </section>
        <dl class="application-ledger">
          <div><dt>审核决定</dt><dd><span class="status-lozenge is-approved">${escapeHtml(data.result.decision)}</span></dd></div>
          <div><dt>应用状态</dt><dd><span class="status-lozenge is-waiting">${escapeHtml(data.result.application)}</span></dd></div>
          <div><dt>当前长期理解</dt><dd>${escapeHtml(data.result.currentTruth)}</dd></div>
        </dl>
        <section class="attention-strip is-warning">
          <div><span class="section-label">关键区别</span><h3>批准不等于已经应用</h3></div>
          <p>只有新的读模型明确返回 applied，OpenLife 才能显示这项变更已完成。</p>
        </section>
        <section class="next-action-band approved-actions">
          <div><span class="section-label">下一步</span><h3>等待可验证的应用结果</h3></div>
          <div>
            ${actionButton(applyAction)}
            <div class="action-row">${otherActions.map(item => actionButton(item, { compact: true })).join("")}</div>
          </div>
        </section>
      </div>
    `;
  }

  function renderLifeModel(data) {
    return `
      <div class="surface-inner lifemodel-surface">
        ${renderPageIntro(data)}
        <nav class="subnav" aria-label="LifeModel 分区">
          ${data.tabs.map((tab, index) => `<span class="${index === 0 ? "is-active" : ""}">${escapeHtml(tab)}</span>`).join("")}
        </nav>
        <section class="lifemodel-summary" aria-labelledby="lifeSummaryTitle">
          <div>
            <span class="section-label">${escapeHtml(data.summary.kicker)}</span>
            <h2 id="lifeSummaryTitle">${escapeHtml(data.summary.title)}</h2>
            <p>${escapeHtml(data.summary.body)}</p>
          </div>
          ${actionRow(data.actions, "lifemodel-actions")}
        </section>
        <section class="model-section" aria-labelledby="currentUnderstandingTitle">
          <header class="section-heading"><div><span class="section-label">已有偏好</span><h3 id="currentUnderstandingTitle">当前可追溯的理解</h3></div></header>
          <div class="model-statements">
            ${data.dimensions
              .map(
                item => `
                  <article>
                    <span class="model-rail"></span>
                    <div><span class="section-label">${escapeHtml(item.label)}</span><h3>${escapeHtml(item.title)}</h3><p>${escapeHtml(item.body)}</p></div>
                    <dl><div><dt>来源</dt><dd>${escapeHtml(item.provenance)}</dd></div><div><dt>状态</dt><dd>${escapeHtml(item.confidence)}</dd></div></dl>
                  </article>
                `
              )
              .join("")}
          </div>
        </section>
        <section class="pending-model-change">
          <div><span class="section-label">待决定建议</span><h3>${escapeHtml(data.pending.title)}</h3><p>${escapeHtml(data.pending.body)}</p></div>
          <span class="status-lozenge is-waiting">未进入当前理解</span>
        </section>
      </div>
    `;
  }

  function mobileSettingsCategoryControl(data) {
    return `
      <label class="mobile-settings-category">
        <span>设置分类</span>
        <select data-mobile-settings-category>
          ${data.categories
            .map(
              item =>
                `<option value="${escapeHtml(item.id)}" ${item.id === settingsCategory ? "selected" : ""}>${escapeHtml(item.label)}</option>`
            )
            .join("")}
        </select>
      </label>
    `;
  }

  function renderSettings(data) {
    if (settingsCategory !== "models") {
      const content = data.categoryContent[settingsCategory];
      return `
        <div class="settings-page-shell">
          <div class="settings-content-column">
            ${mobileSettingsCategoryControl(data)}
            <header class="settings-page-heading">
              <span class="section-label">设置</span>
              <h2>${escapeHtml(content.title)}</h2>
              <p>${escapeHtml(content.summary)}</p>
            </header>
            <section class="settings-unavailable" role="status">
              <h3>本轮只冻结信息架构与入口</h3>
              <p>该分类没有伪造可保存控件。Phase 4 只有在对应读模型、动作和验证路径明确后才逐项实现。</p>
              <button type="button" class="action-button is-secondary" data-settings-category-jump="models">查看模型与供应商交互</button>
            </section>
            <section class="support-band">
              <div><span class="section-label">字段来源</span><h3>不从页面本地推导产品真值</h3><p>相关能力与当前缺口记录在 Phase 3F 后端功能地图和字段来源表。</p></div>
              <button type="button" class="action-button is-secondary" data-open-technical="true">查看技术详情</button>
            </section>
          </div>
        </div>
      `;
    }

    const statusCopy = {
      clean: { label: "没有未保存更改", tone: "neutral" },
      editing: { label: "设置已修改，传输边界待重新确认", tone: "warning" },
      testing: { label: "正在执行静态连接测试…", tone: "neutral" },
      test_succeeded: { label: "本次静态连接验证成功，设置尚未保存", tone: "success" },
      saving: { label: "正在保存静态设置…", tone: "neutral" },
      saved_pending_refresh: { label: "设置已保存，正在刷新边界…", tone: "warning" },
      boundary_unknown: { label: "设置已保存，传输边界仍为未知", tone: "warning" },
    }[settingsState];
    const testAction = data.actions.find(item => item.id === "settings:test-provider");
    const saveAction = data.actions.find(item => item.id === "settings:save-provider");
    const busy =
      settingsState === "testing" ||
      settingsState === "saving" ||
      settingsState === "saved_pending_refresh";
    return `
      <div class="settings-page-shell">
        <div class="settings-content-column">
          ${mobileSettingsCategoryControl(data)}
          <header class="settings-page-heading">
            <span class="section-label">${escapeHtml(data.hero.kicker)}</span>
            <h2 id="settingsHeroTitle">模型与供应商</h2>
            <p>${escapeHtml(data.hero.body)}</p>
          </header>

          <section class="settings-boundary-summary" aria-labelledby="settingsBoundaryTitle">
            <div>
              <span class="status-dot is-warning" aria-hidden="true"></span>
              <div><h3 id="settingsBoundaryTitle">传输边界待确认</h3><p>配置本地优先不等于当前请求一定留在本地。</p></div>
            </div>
            ${actionButton(
              data.actions.find(item => item.id === "settings:view-privacy"),
              { compact: true }
            )}
          </section>

          <p class="settings-form-status is-${escapeHtml(statusCopy.tone)}" role="status">${escapeHtml(statusCopy.label)}</p>

          <section class="settings-section" aria-labelledby="routingTitle">
            <header><h3 id="routingTitle">模型路由偏好</h3><p>这是配置偏好，不是当前请求路线证明。</p></header>
            <label class="settings-toggle-row">
              <span><strong>优先尝试本地模型</strong><small>本地不可用时不会绕过网络策略或确认流程。</small></span>
              <input type="checkbox" data-settings-field="preferLocal" ${settingsDraft.preferLocal ? "checked" : ""} ${busy ? "disabled" : ""} />
            </label>
          </section>

          <section class="settings-section" aria-labelledby="providerTitle">
            <header><h3 id="providerTitle">云端供应商</h3><p>连接测试会产生一次外部请求；测试成功不会自动保存。</p></header>
            <div class="settings-field-grid">
              <label><span>供应商</span><select data-settings-field="provider" ${busy ? "disabled" : ""}>
                <option value="deepseek" ${settingsDraft.provider === "deepseek" ? "selected" : ""}>DeepSeek</option>
                <option value="openai-compatible" ${settingsDraft.provider === "openai-compatible" ? "selected" : ""}>OpenAI-compatible</option>
                <option value="local" ${settingsDraft.provider === "local" ? "selected" : ""}>本地适配器</option>
              </select></label>
              <label><span>模型</span><input type="text" value="${escapeHtml(settingsDraft.model)}" data-settings-field="model" ${busy ? "disabled" : ""} /></label>
              <label class="is-wide"><span>Base URL</span><input type="url" value="${escapeHtml(settingsDraft.endpoint)}" data-settings-field="endpoint" ${busy ? "disabled" : ""} /></label>
              <label class="is-wide"><span>API Key</span><input type="password" value="${escapeHtml(settingsDraft.credential)}" placeholder="${escapeHtml(settingsDraft.credentialPlaceholder)}" autocomplete="off" data-settings-field="credential" ${busy ? "disabled" : ""} /><small>搜索设置不会读取或匹配这个值。</small></label>
            </div>
            <div class="settings-command-row">
              ${actionButton(busy ? { ...testAction, enabled: false, disabledReason: "当前操作完成后再测试" } : testAction, { compact: true })}
              ${actionButton(busy ? { ...saveAction, enabled: false, disabledReason: "当前操作尚未完成" } : saveAction, { compact: true })}
            </div>
          </section>

          ${
            settingsState === "test_succeeded"
              ? `<section class="connection-result" aria-labelledby="connectionResultTitle"><span class="status-dot is-success" aria-hidden="true"></span><div><h3 id="connectionResultTitle">本次连接验证成功</h3><p>设置尚未保存。静态回执：供应商 ${escapeHtml(settingsDraft.provider)}，模型 ${escapeHtml(settingsDraft.model)}。本次结果也不证明未来请求路线。</p></div></section>`
              : ""
          }

          <details class="settings-advanced">
            <summary>高级字段与验证依据</summary>
            <dl><div><dt>配置来源</dt><dd><code>get_config / save_config</code></dd></div><div><dt>测试结果</dt><dd><code>LlmConnectionTestResult</code></dd></div><div><dt>边界来源</dt><dd><code>ProviderPrivacyBoundarySummary</code></dd></div></dl>
          </details>
        </div>
      </div>
    `;
  }

  function renderWorkSurface() {
    const data = screen();
    nodes.workSurface.className = `work-surface layout-${data.layout}`;
    if (data.layout === "today" || data.layout === "today-stale") {
      nodes.workSurface.innerHTML = renderToday(data);
    } else if (data.layout === "workspace") {
      nodes.workSurface.innerHTML = renderWorkspace(data);
    } else if (data.layout === "tasks") {
      nodes.workSurface.innerHTML = renderTasks(data);
    } else if (data.layout === "review") {
      nodes.workSurface.innerHTML = renderReview(data);
    } else if (data.layout === "review-approved") {
      nodes.workSurface.innerHTML = renderReviewApproved(data);
    } else if (data.layout === "lifemodel") {
      nodes.workSurface.innerHTML = renderLifeModel(data);
    } else if (data.layout === "settings") {
      nodes.workSurface.innerHTML = renderSettings(data);
    }
    bindSurfaceInteractions();
  }

  function renderInspector(data, openTechnical = false) {
    const inspector = data.inspector;
    const permission = data.permission;
    nodes.inspectorBody.innerHTML = `
      <section class="inspector-conclusion" aria-labelledby="inspectorConclusionTitle">
        <span class="section-label">先看结论</span>
        <h3 id="inspectorConclusionTitle">发生了什么</h3>
        <dl>
          <div><dt>当前情况</dt><dd>${escapeHtml(inspector.summary.happened)}</dd></div>
          <div><dt>主要风险</dt><dd>${escapeHtml(inspector.summary.risk)}</dd></div>
          <div><dt>你可以做什么</dt><dd>${escapeHtml(inspector.summary.next)}</dd></div>
        </dl>
      </section>

      ${
        inspector.permission && permission
          ? `
            <section class="inspector-section permission-detail" aria-labelledby="permissionDetailTitle">
              <span class="section-label">访问范围</span>
              <h3 id="permissionDetailTitle">${escapeHtml(permission.title)}</h3>
              <p>${escapeHtml(permission.purpose)}</p>
              <dl>
                <div><dt>工具</dt><dd>${escapeHtml(permission.tool)}</dd></div>
                <div><dt>能力</dt><dd>${escapeHtml(permission.capability)}</dd></div>
                <div><dt>目标</dt><dd>${escapeHtml(permission.target)}</dd></div>
                <div><dt>数据</dt><dd>${escapeHtml(permission.dataScope)}</dd></div>
                <div><dt>外传</dt><dd class="is-warning">${escapeHtml(permission.transmission)}</dd></div>
                <div><dt>时效</dt><dd>${escapeHtml(permission.duration)}</dd></div>
                <div><dt>撤销</dt><dd>${escapeHtml(permission.revocation)}</dd></div>
              </dl>
            </section>
          `
          : ""
      }

      <section class="inspector-section privacy-section" aria-labelledby="privacyTitle">
        <span class="section-label">模型与隐私</span>
        <h3 id="privacyTitle">${escapeHtml(inspector.privacy.title)}</h3>
        <div class="privacy-callout"><span class="status-dot is-${escapeHtml(inspector.privacy.tone || "warning")}" aria-hidden="true"></span><p>${escapeHtml(inspector.privacy.meta)}</p></div>
      </section>

      <section class="inspector-section" aria-labelledby="evidenceTitle">
        <span class="section-label">参考依据</span>
        <h3 id="evidenceTitle">判断来自哪里</h3>
        <div class="evidence-list">
          ${inspector.evidence
            .map(
              item => `
                <article class="evidence-item" tabindex="0" data-evidence-id="${escapeHtml(item.id)}">
                  <div><strong>${escapeHtml(item.label)}</strong><span>${escapeHtml(item.sensitivity)}</span></div>
                  <p>${escapeHtml(item.summary)}</p>
                  <dl><div><dt>来源</dt><dd>${escapeHtml(item.source)}</dd></div><div><dt>ID</dt><dd><code>${escapeHtml(item.id)}</code></dd></div></dl>
                </article>
              `
            )
            .join("")}
        </div>
      </section>

      <section class="inspector-section limitations-section" aria-labelledby="limitationsTitle">
        <span class="section-label">限制</span>
        <h3 id="limitationsTitle">当前不能证明的内容</h3>
        <ul>${inspector.limitations.map(item => `<li>${escapeHtml(item)}</li>`).join("")}</ul>
      </section>

      <details class="technical-details" ${openTechnical ? "open" : ""}>
        <summary>技术详情</summary>
        <dl>
          ${Object.entries(inspector.technical)
            .map(
              ([key, value]) =>
                `<div><dt>${escapeHtml(key)}</dt><dd><code>${escapeHtml(typeof value === "string" ? value : JSON.stringify(value))}</code></dd></div>`
            )
            .join("")}
        </dl>
        <p>这里是视觉 fixture 的字段来源，不是生产调试状态。</p>
      </details>
    `;
  }

  function bindSurfaceInteractions() {
    $$("[data-action-id]", nodes.workSurface).forEach(button => {
      button.addEventListener("click", () => {
        const data = screen();
        const item = data.actions?.find(
          candidate => candidate.id === button.getAttribute("data-action-id")
        );
        if (item) handleAction(item, button);
      });
    });

    $$("[data-task-filter]", nodes.workSurface).forEach(button => {
      button.addEventListener("click", () => {
        activeTaskFilter = button.getAttribute("data-task-filter");
        renderWorkSurface();
        announce(`任务筛选已切换为${activeTaskFilter}`);
      });
    });

    $$("[data-task-id]", nodes.workSurface).forEach(button => {
      button.addEventListener("click", () => {
        selectedTaskId = button.getAttribute("data-task-id");
        renderWorkSurface();
        const selected = screen().taskItems.find(item => item.id === selectedTaskId);
        announce(`已选择任务：${selected?.title || ""}`);
      });
    });

    $$("[data-review-queue-id]", nodes.workSurface).forEach(button => {
      button.addEventListener("click", () => {
        const target = button.getAttribute("data-review-queue-id");
        if (target === "review:permission:hangzhou-files") {
          setScreen("workspace", { focus: true, announce: true });
        }
      });
    });

    $$("[data-static-feedback]", nodes.workSurface).forEach(button => {
      button.addEventListener("click", () => {
        const type = button.getAttribute("data-static-feedback");
        if (type === "attach") {
          startFixtureImport();
        } else {
          const textarea = $("#workspaceComposer");
          showToast(
            textarea?.value.trim()
              ? "静态原型：补充说明只保留在当前页面，未发送。"
              : "请输入补充说明后再发送。"
          );
        }
      });
    });

    $$("[data-resource-remove]", nodes.workSurface).forEach(button => {
      button.addEventListener("click", () => {
        const id = button.getAttribute("data-resource-remove");
        const fixtureIndex = fixtureAttachments.findIndex(item => item.id === id);
        if (fixtureIndex >= 0) fixtureAttachments.splice(fixtureIndex, 1);
        else detachedResourceIds.add(id);
        renderWorkSurface();
        showToast("静态演示：附件绑定已移除，没有删除原始文件。");
      });
    });

    $$("[data-settings-field]", nodes.workSurface).forEach(field => {
      field.addEventListener("change", () => {
        const key = field.getAttribute("data-settings-field");
        settingsDraft[key] = field.type === "checkbox" ? field.checked : field.value;
        settingsState = "editing";
        renderStatus({ label: "配置已修改，边界待重新确认", tone: "warning" });
        renderWorkSurface();
        $(`[data-settings-field="${key}"]`, nodes.workSurface)?.focus({ preventScroll: true });
        announce("设置已修改；当前传输边界保持未知");
      });
    });

    $("[data-settings-category-jump]", nodes.workSurface)?.addEventListener("click", () => {
      settingsCategory = "models";
      renderNavigation();
      renderWorkSurface();
      nodes.workSurface.focus({ preventScroll: true });
      announce("已切换到模型与供应商设置");
    });

    $("[data-mobile-settings-category]", nodes.workSurface)?.addEventListener("change", event => {
      const label = event.currentTarget.selectedOptions[0].textContent;
      settingsCategory = event.currentTarget.value;
      renderNavigation();
      renderWorkSurface();
      nodes.workSurface.focus({ preventScroll: true });
      announce(`设置分类已切换为${label}`);
    });

    $$("[data-open-technical]", nodes.workSurface).forEach(button => {
      button.addEventListener("click", () => openInspector(button, true));
    });
  }

  function startFixtureImport() {
    if (fixtureAttachments.some(item => item.state === "importing")) return;
    const id = `res:fixture:${Date.now()}`;
    fixtureAttachments.push({
      id,
      name: "餐厅预订确认.pdf",
      state: "importing",
      meta: "静态导入中",
    });
    renderWorkSurface();
    announce("静态演示：附件导入开始");
    window.setTimeout(() => {
      const item = fixtureAttachments.find(candidate => candidate.id === id);
      if (!item) return;
      item.state = "ready";
      item.meta = "静态回执已提交";
      renderWorkSurface();
      showToast("静态演示：附件导入回执已提交；没有读取真实文件。");
    }, 650);
  }

  function handleAction(item, trigger) {
    if (!item.enabled) return;
    if (item.outcome === "inspector") {
      openInspector(trigger);
      return;
    }
    if (model.screens[item.outcome]) {
      setScreen(item.outcome, { focus: true, announce: true });
      return;
    }
    if (item.outcome === "review-confirm-approve") {
      openDecisionDialog({
        kicker: "确认审核决定",
        title: "批准这项长期偏好？",
        body: "批准只记录你的决定，不代表偏好已经应用到 LifeModel。",
        tone: "warning",
        confirmLabel: "确认批准",
        onConfirm: () => setScreen("review-approved", { focus: true, announce: true }),
      });
      return;
    }

    if (item.outcome === "permission-confirm-and-resume") {
      const permission = screen().permission;
      openDecisionDialog({
        kicker: "确认一次性权限",
        title: "只允许当前文件读取并继续？",
        bodyHtml: `
          <dl class="dialog-scope">
            <div><dt>目的</dt><dd>${escapeHtml(permission.purpose)}</dd></div>
            <div><dt>目标</dt><dd>${escapeHtml(permission.target)}</dd></div>
            <div><dt>范围</dt><dd>${escapeHtml(permission.dataScope)}</dd></div>
            <div><dt>传输</dt><dd>${escapeHtml(permission.transmission)}</dd></div>
            <div><dt>有效性</dt><dd>${escapeHtml(permission.duration)}</dd></div>
          </dl>
          <p>静态演示会依次表现：记录决定、刷新审核与任务、核对恢复动作、请求恢复。任何一步不匹配都应继续暂停。</p>
        `,
        tone: "warning",
        confirmLabel: "仅允许本次并继续",
        onConfirm: startPermissionResumeFixture,
      });
      return;
    }

    if (item.outcome === "permission-reject") {
      permissionFlowToken += 1;
      permissionFlowStage = "rejected";
      renderStatus({ label: "已拒绝访问，任务保持暂停", tone: "warning" });
      renderWorkSurface();
      showToast("静态演示：已拒绝本次访问；没有读取文件。");
      return;
    }

    if (item.outcome === "review-reject") {
      reviewFixtureState = "rejected";
      renderStatus({ label: "建议已拒绝，未应用", tone: "neutral" });
      renderWorkSurface();
      announce("静态演示：建议已拒绝，当前长期状态保持不变");
      return;
    }

    if (item.outcome === "review-later") {
      reviewFixtureState = "postponed";
      renderStatus({ label: "已设为稍后处理，未批准", tone: "warning" });
      renderWorkSurface();
      announce("静态演示：建议已设为稍后处理，没有批准或应用");
      return;
    }

    if (item.outcome === "review-edit") {
      openDecisionDialog({
        kicker: "修改建议",
        title: "调整建议内容",
        bodyHtml: `
          <label class="dialog-field" for="reviewEditValue">
            <span>建议内容</span>
            <textarea id="reviewEditValue" rows="3">${escapeHtml(reviewEditedAfter || screen().proposal.after)}</textarea>
          </label>
          <p>保存后仍是等待决定；修改本身不会批准或应用。</p>
        `,
        tone: "neutral",
        confirmLabel: "保存修改",
        onConfirm: () => {
          const value = $("#reviewEditValue", nodes.dialog)?.value.trim();
          if (!value) return;
          reviewEditedAfter = value;
          reviewFixtureState = "pending";
          renderStatus({ label: "建议已修改，仍待决定", tone: "warning" });
          renderWorkSurface();
          announce("静态演示：建议已修改，状态仍为等待决定");
        },
      });
      return;
    }

    if (item.outcome === "settings-test-connection") {
      openDecisionDialog({
        kicker: "外部连接确认",
        title: "测试这组供应商设置？",
        body: `静态目标：${settingsDraft.provider} · ${settingsDraft.model} · ${settingsDraft.endpoint}。真实产品会发起一次外部请求；测试不会保存配置。`,
        tone: "warning",
        confirmLabel: "开始静态测试",
        onConfirm: startSettingsTestFixture,
      });
      return;
    }

    if (item.outcome === "settings-save") {
      startSettingsSaveFixture();
      return;
    }

    if (item.outcome === "cancel-task-feedback") {
      openDecisionDialog({
        kicker: "取消当前任务",
        title: "停止当前处理？",
        body: "静态演示只验证确认与反馈。真实产品必须刷新任务状态，并在外部动作已发出时允许出现“外部结果未知”。",
        tone: "warning",
        confirmLabel: "确认静态取消",
        onConfirm: () => showToast("静态演示：已提交取消；没有调用真实任务命令。"),
      });
      return;
    }

    const feedback = {
      "refresh-feedback": {
        title: "刷新请求未连接",
        body: "这是静态原型。状态继续保持陈旧，不会为了展示而跳到可用状态。",
      },
    }[item.outcome] || {
      title: "静态原型反馈",
      body: "该交互只验证界面结果，没有连接生产命令。",
    };

    openDecisionDialog({
      kicker: "静态原型反馈",
      title: feedback.title,
      body: feedback.body,
      tone: "neutral",
      confirmLabel: "知道了",
    });
  }

  function startPermissionResumeFixture() {
    const token = ++permissionFlowToken;
    const advance = (stage, label, delay, next) => {
      if (token !== permissionFlowToken || currentScreenKey !== "workspace") return;
      permissionFlowStage = stage;
      renderStatus({ label, tone: "warning" });
      renderWorkSurface();
      announce(`静态演示：${label}`);
      window.setTimeout(next, delay);
    };
    advance("recording", "正在记录一次性决定", 500, () =>
      advance("refreshing", "决定已返回，正在刷新状态", 500, () =>
        advance("resuming", "范围已核对，正在请求恢复", 500, () => {
          if (token !== permissionFlowToken || currentScreenKey !== "workspace") return;
          permissionFlowStage = "idle";
          setScreen("workspace-running", { focus: true, announce: true });
          showToast("静态演示：刷新后的任务状态为正在处理。");
        })
      )
    );
  }

  function startSettingsTestFixture() {
    settingsState = "testing";
    renderStatus({ label: "正在执行静态连接测试", tone: "warning" });
    renderWorkSurface();
    announce("静态演示：连接测试开始；设置尚未保存");
    window.setTimeout(() => {
      if (currentScreenKey !== "settings") return;
      settingsState = "test_succeeded";
      renderStatus({ label: "本次连接验证成功，设置尚未保存", tone: "success" });
      renderWorkSurface();
      announce("静态演示：本次连接验证成功；设置仍未保存，未来路线仍未证明");
    }, 750);
  }

  function startSettingsSaveFixture() {
    settingsState = "saving";
    renderStatus({ label: "正在保存静态设置", tone: "warning" });
    renderWorkSurface();
    announce("静态演示：正在保存设置");
    window.setTimeout(() => {
      if (currentScreenKey !== "settings") return;
      settingsState = "saved_pending_refresh";
      renderStatus({ label: "设置已保存，正在刷新边界", tone: "warning" });
      renderWorkSurface();
      announce("静态演示：设置已保存，正在等待后端边界刷新");
      window.setTimeout(() => {
        if (currentScreenKey !== "settings") return;
        settingsState = "boundary_unknown";
        renderStatus({ label: "设置已保存，传输边界仍为未知", tone: "warning" });
        renderWorkSurface();
        announce("静态演示：边界刷新结果仍为未知，没有显示本地确定态");
      }, 650);
    }, 650);
  }

  function openDecisionDialog(config) {
    dialogTrigger = document.activeElement;
    nodes.dialogKicker.textContent = config.kicker;
    nodes.dialogTitle.textContent = config.title;
    if (config.bodyHtml) nodes.dialogBody.innerHTML = config.bodyHtml;
    else nodes.dialogBody.textContent = config.body;
    nodes.dialogIcon.className = `dialog-icon is-${config.tone}`;
    nodes.dialogIcon.innerHTML = config.tone === "warning" ? "!" : icon("shield");
    nodes.dialogActions.innerHTML = config.onConfirm
      ? `<button class="action-button is-secondary" type="button" data-dialog-cancel>取消</button><button class="action-button is-primary" type="button" data-dialog-confirm>${escapeHtml(config.confirmLabel)}</button>`
      : `<button class="action-button is-primary" type="button" data-dialog-confirm>${escapeHtml(config.confirmLabel)}</button>`;
    const confirm = $("[data-dialog-confirm]", nodes.dialogActions);
    const cancel = $("[data-dialog-cancel]", nodes.dialogActions);
    confirm.addEventListener("click", () => {
      nodes.dialog.close();
      config.onConfirm?.();
    });
    cancel?.addEventListener("click", () => {
      nodes.dialog.close();
      dialogTrigger?.focus?.();
    });
    nodes.dialog.showModal();
    requestAnimationFrame(() => (cancel || $("textarea", nodes.dialog) || confirm).focus());
  }

  function showToast(message) {
    window.clearTimeout(toastTimer);
    nodes.toast.textContent = message;
    nodes.toast.hidden = false;
    nodes.toast.classList.add("is-visible");
    announce(message);
    toastTimer = window.setTimeout(() => {
      nodes.toast.classList.remove("is-visible");
      nodes.toast.hidden = true;
    }, 3200);
  }

  function isMobile() {
    return window.matchMedia("(max-width: 760px)").matches;
  }

  function openInspector(trigger, openTechnical = false) {
    inspectorTrigger = trigger || document.activeElement;
    inspectorOpen = true;
    renderInspector(screen(), openTechnical);
    nodes.shell.classList.add("is-inspector-open");
    nodes.inspector.setAttribute("aria-hidden", "false");
    nodes.inspectorBackdrop.hidden = !isMobile();
    requestAnimationFrame(() => nodes.closeInspector.focus());
    announce("依据与边界面板已打开");
  }

  function closeInspector(options = {}) {
    if (!inspectorOpen) return;
    inspectorOpen = false;
    nodes.shell.classList.remove("is-inspector-open");
    nodes.inspector.setAttribute("aria-hidden", "true");
    nodes.inspectorBackdrop.hidden = true;
    if (options.restoreFocus !== false && inspectorTrigger?.focus) inspectorTrigger.focus();
    announce("依据与边界面板已关闭");
  }

  function setScreen(key, options = {}) {
    if (!model.screens[key]) return;
    currentScreenKey = key;
    closeInspector({ restoreFocus: false });
    nodes.select.value = key;
    const data = screen();
    nodes.contextEyebrow.textContent = data.subtitle;
    nodes.contextTitle.textContent = data.title;
    renderStatus(data.status);
    renderPrivacy(data.privacy);
    renderNavigation();
    renderWorkSurface();
    renderInspector(data);
    const url = new URL(window.location.href);
    url.searchParams.set("screen", key);
    window.history.replaceState({}, "", url);
    if (options.focus) nodes.workSurface.focus({ preventScroll: true });
    if (options.announce) announce(`已切换到${data.selectorLabel}`);
  }

  function resetScenarioState(key) {
    if (key === "review-pending") {
      reviewFixtureState = "pending";
      reviewEditedAfter = "";
    }
    if (key === "workspace" || key === "workspace-unknown") {
      permissionFlowToken += 1;
      permissionFlowStage = "idle";
    }
    if (key === "settings") {
      settingsCategory = "models";
      settingsSearchQuery = "";
      settingsState = "clean";
      settingsDraft = { ...model.screens.settings.config };
    }
    fixtureAttachments = [];
    detachedResourceIds.clear();
  }

  function trapInspectorFocus(event) {
    if (!inspectorOpen || !isMobile() || event.key !== "Tab") return;
    const focusable = $$('button:not([disabled]), summary, [tabindex="0"]', nodes.inspector).filter(
      node => node.offsetParent !== null
    );
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

  function initialize() {
    nodes.select.innerHTML = Object.values(model.screens)
      .map(
        item => `<option value="${escapeHtml(item.key)}">${escapeHtml(item.selectorLabel)}</option>`
      )
      .join("");
    nodes.select.addEventListener("change", () => {
      resetScenarioState(nodes.select.value);
      setScreen(nodes.select.value, { focus: true, announce: true });
    });

    nodes.openInspector.addEventListener("click", () => openInspector(nodes.openInspector));
    nodes.closeInspector.addEventListener("click", () => closeInspector());
    nodes.inspectorBackdrop.addEventListener("click", () => closeInspector());
    nodes.privacyBoundaryButton.addEventListener("click", () =>
      openInspector(nodes.privacyBoundaryButton)
    );
    nodes.openMobileMenu.addEventListener("click", () => nodes.mobileDrawer.showModal());
    nodes.closeMobileMenu.addEventListener("click", () => nodes.mobileDrawer.close());

    document.addEventListener("keydown", event => {
      if (event.key === "Escape" && inspectorOpen) {
        event.preventDefault();
        closeInspector();
        return;
      }
      trapInspectorFocus(event);
    });

    const requested = new URLSearchParams(window.location.search).get("screen");
    setScreen(model.screens[requested] ? requested : model.defaultScreen);
  }

  window.__OPENLIFE_BLUEPRINT__ = {
    setScreen: key => setScreen(key, { focus: false, announce: false }),
    openInspector: () => openInspector(nodes.openInspector),
    closeInspector,
    getState: () => ({ currentScreenKey, inspectorOpen, activeTaskFilter, selectedTaskId }),
  };

  initialize();
})();
