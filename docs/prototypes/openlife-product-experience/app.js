(() => {
  const data = window.OPENLIFE_FIXTURES;
  if (!data) throw new Error("OpenLife prototype fixtures are unavailable.");

  const profiles = [
    {
      id: "local",
      name: "Local Ollama",
      meta: "本地 · 可用",
      tone: "success",
      models: ["llama3.1", "qwen2.5-coder:14b"],
      summary: "请求保留在本机。Chat 与基础 Work 已验证。",
    },
    {
      id: "cloud",
      name: "Cloud Profile",
      meta: "云端 · 配额受限",
      tone: "warning",
      models: ["Work model", "Chat model"],
      summary: "请求会离开设备；当前 Work 模型接近配额限制。",
    },
    {
      id: "compatible",
      name: "Compatible API",
      meta: "自定义端点 · 未验证",
      tone: "neutral",
      models: ["custom-model"],
      summary: "需要先完成连接、流式响应与结构化步骤验证。",
    },
  ];

  const folders = [
    { id: "open-life", name: "open-life", path: "/Users/tw/Desktop/open-life", status: "可用", tone: "success" },
    { id: "research", name: "research", path: "/Users/tw/Documents/research", status: "需要授权", tone: "warning" },
    { id: "campaign", name: "campaign", path: "/Volumes/Archive/campaign", status: "目录已失效", tone: "danger" },
  ];

  const diffFiles = [
    {
      name: "launch-brief.md",
      kind: "新增",
      summary: "+8 / -0",
      lines: [
        ["add", "+", "# Launch brief"],
        ["add", "+", ""],
        ["add", "+", "## Goal"],
        ["add", "+", "Ship a focused desktop Agent experience."],
        ["add", "+", ""],
        ["add", "+", "## Acceptance"],
        ["add", "+", "- Provider and model are selectable"],
        ["add", "+", "- Project changes remain reversible"],
      ],
    },
    {
      name: "README.md",
      kind: "修改",
      summary: "+2 / -1",
      lines: [
        ["context", " ", "## Product"],
        ["remove", "-", "See docs/overview.md for the current release."],
        ["add", "+", "See launch-brief.md for the current release goal."],
        ["add", "+", "Project work is previewed before changes are applied."],
      ],
    },
    {
      name: "draft.txt → research-notes.txt",
      kind: "重命名",
      summary: "内容未变",
      lines: [
        ["context", " ", "File renamed without content changes."],
        ["context", " ", "Previous path: draft.txt"],
        ["context", " ", "New path: research-notes.txt"],
      ],
    },
  ];

  const byId = id => document.getElementById(id);
  const dom = {
    appShell: byId("app-shell"),
    sidebar: document.querySelector(".sidebar"),
    controls: byId("prototype-controls"),
    controlsToggle: byId("toggle-prototype-controls"),
    controlsClose: byId("close-prototype-controls"),
    journeySelect: byId("journey-select"),
    journeyCounter: byId("journey-counter"),
    stateTitle: byId("state-title"),
    previousState: byId("previous-state"),
    nextState: byId("next-state"),
    projectList: byId("project-list"),
    conversationList: byId("conversation-list"),
    sidebarSections: byId("sidebar-sections"),
    newChatButton: byId("new-chat-button"),
    settingsButton: byId("settings-button"),
    contextEyebrow: byId("context-eyebrow"),
    contextTitle: byId("context-title"),
    contextStatus: byId("context-status"),
    threadIntro: byId("thread-intro"),
    messageList: byId("message-list"),
    workProgress: byId("work-progress"),
    inlineDecision: byId("inline-decision"),
    resultCard: byId("result-card"),
    modeChat: byId("mode-chat"),
    modeWork: byId("mode-work"),
    profileButton: byId("profile-button"),
    resourceChips: byId("resource-chips"),
    composer: byId("composer"),
    composerWrap: document.querySelector(".composer-wrap"),
    composerInput: byId("composer-input"),
    sendButton: byId("send-button"),
    attachButton: byId("attach-button"),
    composerTools: document.querySelector(".composer-tools"),
    composerActions: document.querySelector(".composer-actions"),
    inspector: byId("inspector"),
    inspectorToggle: byId("inspector-toggle"),
    inspectorClose: byId("inspector-close"),
    inspectorEyebrow: byId("inspector-eyebrow"),
    inspectorTitle: byId("inspector-title"),
    inspectorBody: byId("inspector-body"),
    sidebarToggle: byId("sidebar-toggle"),
    scrim: byId("scrim"),
    thread: byId("thread"),
    modelPicker: byId("model-picker"),
    modelPickerClose: byId("model-picker-close"),
    modelPickerCancel: byId("model-picker-cancel"),
    profileOptions: byId("profile-options"),
    modelSelect: byId("model-select"),
    reasoningOptions: byId("reasoning-options"),
    profileSummary: byId("profile-summary"),
    useModel: byId("use-model"),
    manageProfiles: byId("manage-profiles"),
    folderPicker: byId("folder-picker"),
    folderPickerClose: byId("folder-picker-close"),
    folderPickerCancel: byId("folder-picker-cancel"),
    folderOptions: byId("folder-options"),
    folderNotice: byId("folder-notice"),
    browseFolder: byId("browse-folder"),
    openSelectedFolder: byId("open-selected-folder"),
    profileManager: byId("profile-manager"),
    profileManagerClose: byId("profile-manager-close"),
    profileManagerList: byId("profile-manager-list"),
    addProfile: byId("add-profile"),
    profileForm: byId("profile-form"),
    providerType: byId("provider-type"),
    profileName: byId("profile-name"),
    profileEndpoint: byId("profile-endpoint"),
    profileCredential: byId("profile-credential"),
    profileValidation: byId("profile-validation"),
    verifyProfile: byId("verify-profile"),
    resourcePicker: byId("resource-picker"),
    resourcePickerClose: byId("resource-picker-close"),
    addLocalFile: byId("add-local-file"),
    addProjectFolder: byId("add-project-folder"),
    resourceUrl: byId("resource-url"),
    addResourceUrl: byId("add-resource-url"),
    resourceNotice: byId("resource-notice"),
  };

  const model = {
    journeyIndex: 0,
    stateIndex: 0,
    inspectorOpen: false,
    sidebarOpen: false,
    controlsVisible: false,
    selectedProfileId: "local",
    selectedModel: "llama3.1",
    reasoning: "中",
    profileOverride: null,
    modeOverride: null,
    selectedFolderId: "open-life",
    folderPermissionGranted: false,
    diffFileIndex: 0,
    diffSelected: new Set([0, 1, 2]),
    diffReviewed: new Set(),
    pendingModelAdvance: false,
    resultOverride: null,
    inspectorOverride: null,
    resourceOverrides: [],
  };

  const el = (tag, className, text) => {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
  };

  const currentJourney = () => data.journeys[model.journeyIndex];
  const currentState = () => currentJourney().states[model.stateIndex];

  function setTone(node, tone) {
    delete node.dataset.tone;
    if (tone) node.dataset.tone = tone;
  }

  function renderJourneyOptions() {
    dom.journeySelect.replaceChildren();
    data.journeys.forEach((journey, index) => {
      const option = document.createElement("option");
      option.value = String(index);
      option.textContent = `${journey.number}. ${journey.title}`;
      dom.journeySelect.append(option);
    });
  }

  function renderSidebar() {
    const journey = currentJourney();
    const view = currentState();
    const firstLaunch = journey.id === "onboarding" && model.stateIndex === 0;
    const isNewChat = journey.id === "chat" && model.stateIndex === 0;
    const projectName =
      view.project ||
      view.resources.map(resource => resource.match(/Project · (?:\/[^·]+\/)?([^·/]+)(?: ·|$)/)?.[1]).find(Boolean) ||
      ({ edit: "campaign", "long-work": "research", review: "finance", recovery: "strategy" }[journey.id] ?? null);

    dom.sidebarSections.hidden = firstLaunch;
    dom.newChatButton.classList.toggle("active", isNewChat);
    dom.newChatButton.setAttribute("aria-current", isNewChat ? "page" : "false");
    dom.projectList.replaceChildren();
    const projectItems = [...data.projects];
    if (projectName && !projectItems.some(project => project.name === projectName)) {
      projectItems.unshift({ name: projectName, meta: "当前" });
    }
    projectItems.forEach(project => {
      const active = project.name === projectName;
      const button = el("button", `nav-row${active ? " active" : ""}`);
      button.type = "button";
      button.dataset.journey = "project";
      button.append(el("span", "", project.name), el("small", "", project.meta));
      if (active) button.setAttribute("aria-current", "page");
      dom.projectList.append(button);
    });

    dom.conversationList.replaceChildren();
    const conversationContext = !["onboarding", "project", "history"].includes(journey.id) && !isNewChat;
    const conversationItems = conversationContext
      ? [{ name: view.contextTitle, meta: view.mode, current: true }, ...data.conversations.filter(item => item.name !== view.contextTitle)]
      : data.conversations;
    conversationItems.forEach(conversation => {
      const button = el("button", `nav-row${conversation.current ? " active" : ""}`);
      button.type = "button";
      button.dataset.journey = conversation.meta === "Chat" ? "chat" : "long-work";
      button.append(el("span", "", conversation.name), el("small", "", conversation.meta));
      if (conversation.current) button.setAttribute("aria-current", "page");
      dom.conversationList.append(button);
    });
  }

  function renderModelPicker() {
    const selectedProfile = profiles.find(profile => profile.id === model.selectedProfileId) || profiles[0];
    dom.profileOptions.replaceChildren();
    profiles.forEach(profile => {
      const selected = profile.id === selectedProfile.id;
      const button = el("button", `picker-row${selected ? " selected" : ""}`);
      button.type = "button";
      button.setAttribute("role", "option");
      button.setAttribute("aria-selected", String(selected));
      button.dataset.profileId = profile.id;
      const copy = el("span", "picker-row-copy");
      copy.append(el("strong", "", profile.name), el("small", "", profile.meta));
      button.append(copy, el("span", `picker-status ${profile.tone}`, profile.status || ""));
      dom.profileOptions.append(button);
    });

    dom.modelSelect.replaceChildren();
    selectedProfile.models.forEach(name => {
      const option = el("option", "", name);
      option.value = name;
      dom.modelSelect.append(option);
    });
    if (!selectedProfile.models.includes(model.selectedModel)) model.selectedModel = selectedProfile.models[0];
    dom.modelSelect.value = model.selectedModel;
    dom.reasoningOptions.querySelectorAll("button").forEach(button => {
      const selected = button.dataset.reasoning === model.reasoning;
      button.classList.toggle("selected", selected);
      button.setAttribute("aria-pressed", String(selected));
    });
    dom.profileSummary.replaceChildren(
      el("strong", "", `${selectedProfile.name} · ${model.selectedModel}`),
      el("p", "", selectedProfile.summary)
    );
    dom.useModel.disabled = selectedProfile.id === "compatible";
    dom.useModel.textContent = selectedProfile.id === "compatible" ? "需要先验证" : "用于本轮";
  }

  function openModelPicker() {
    model.selectedProfileId = model.profileOverride?.profileId || (currentState().profile.startsWith("Cloud") ? "cloud" : "local");
    model.selectedModel = model.profileOverride?.model || (model.selectedProfileId === "cloud" ? "Work model" : "llama3.1");
    renderModelPicker();
    dom.modelPicker.showModal();
  }

  function closeModelPicker() {
    if (dom.modelPicker.open) dom.modelPicker.close();
    dom.profileButton.focus();
  }

  function renderProfileManager() {
    dom.profileManagerList.replaceChildren();
    profiles.forEach(profile => {
      const row = el("div", "profile-manager-row");
      const copy = el("div", "picker-row-copy");
      copy.append(el("strong", "", profile.name), el("small", "", profile.meta));
      row.append(copy, el("span", `folder-status ${profile.tone}`, profile.tone === "success" ? "已验证" : profile.tone === "warning" ? "需关注" : "未验证"));
      dom.profileManagerList.append(row);
    });
  }

  function openProfileManager() {
    if (dom.modelPicker.open) dom.modelPicker.close();
    renderProfileManager();
    dom.profileForm.hidden = true;
    dom.profileValidation.textContent = "尚未验证";
    dom.profileManager.showModal();
  }

  function closeProfileManager() {
    if (dom.profileManager.open) dom.profileManager.close();
  }

  function openResourcePicker() {
    dom.resourceNotice.textContent = "不会在未确认时扩大到其他文件或网站。";
    dom.resourcePicker.showModal();
  }

  function closeResourcePicker() {
    if (dom.resourcePicker.open) dom.resourcePicker.close();
  }

  function addConversationResource(label) {
    if (!model.resourceOverrides.includes(label)) model.resourceOverrides.push(label);
    closeResourcePicker();
    render();
  }

  function renderFolderPicker() {
    dom.folderOptions.replaceChildren();
    folders.forEach(folder => {
      const selected = folder.id === model.selectedFolderId;
      const button = el("button", `picker-row${selected ? " selected" : ""}`);
      button.type = "button";
      button.setAttribute("role", "option");
      button.setAttribute("aria-selected", String(selected));
      button.dataset.folderId = folder.id;
      const copy = el("span", "picker-row-copy");
      copy.append(el("strong", "", folder.name), el("small", "path", folder.path));
      button.append(copy, el("span", `folder-status ${folder.tone}`, folder.status));
      dom.folderOptions.append(button);
    });
    const selectedFolder = folders.find(folder => folder.id === model.selectedFolderId) || folders[0];
    if (selectedFolder.tone === "danger") {
      dom.folderNotice.textContent = "这个目录已移动或断开连接。请重新定位后再打开。";
      dom.openSelectedFolder.textContent = "重新定位";
      dom.openSelectedFolder.disabled = true;
    } else if (selectedFolder.tone === "warning" && !model.folderPermissionGranted) {
      dom.folderNotice.textContent = "OpenLife 需要重新获得此目录的访问权限。";
      dom.openSelectedFolder.textContent = "授权并打开";
      dom.openSelectedFolder.disabled = false;
    } else {
      dom.folderNotice.textContent = `${selectedFolder.path} · 当前可用`;
      dom.openSelectedFolder.textContent = "打开 Project";
      dom.openSelectedFolder.disabled = false;
    }
  }

  function openFolderPicker() {
    model.selectedFolderId = "open-life";
    model.folderPermissionGranted = false;
    renderFolderPicker();
    dom.folderPicker.showModal();
  }

  function closeFolderPicker() {
    if (dom.folderPicker.open) dom.folderPicker.close();
  }

  function renderDiffInspector() {
    dom.inspectorEyebrow.textContent = "Diff · 3 files";
    dom.inspectorTitle.textContent = "审查更改";
    dom.inspectorBody.replaceChildren();

    const layout = el("div", "diff-review");
    const fileList = el("div", "diff-file-list");
    diffFiles.forEach((file, index) => {
      const row = el("div", `diff-file-row${index === model.diffFileIndex ? " active" : ""}`);
      const checkbox = document.createElement("input");
      checkbox.type = "checkbox";
      checkbox.checked = model.diffSelected.has(index);
      checkbox.setAttribute("aria-label", `选择 ${file.name}`);
      checkbox.addEventListener("change", () => {
        if (checkbox.checked) model.diffSelected.add(index);
        else model.diffSelected.delete(index);
        renderDiffInspector();
      });
      const button = el("button", "diff-file-button");
      button.type = "button";
      button.append(
        el("strong", "", file.name),
        el("small", "", `${file.kind} · ${file.summary}${model.diffReviewed.has(index) ? " · 已审查" : ""}`)
      );
      button.addEventListener("click", () => {
        model.diffFileIndex = index;
        renderDiffInspector();
      });
      row.append(checkbox, button);
      fileList.append(row);
    });

    const currentFile = diffFiles[model.diffFileIndex];
    const content = el("section", "diff-content");
    const header = el("div", "diff-content-header");
    header.append(el("strong", "", currentFile.name), el("span", "", currentFile.summary));
    const code = el("div", "diff-lines");
    currentFile.lines.forEach((line, index) => {
      const row = el("div", `diff-line ${line[0]}`);
      row.append(el("span", "diff-line-number", String(index + 1)), el("span", "diff-line-sign", line[1]), el("code", "", line[2]));
      code.append(row);
    });
    const markReviewed = el(
      "button",
      model.diffReviewed.has(model.diffFileIndex) ? "secondary-button" : "primary-button",
      model.diffReviewed.has(model.diffFileIndex) ? "已审查" : "标记为已审查"
    );
    markReviewed.type = "button";
    markReviewed.disabled = model.diffReviewed.has(model.diffFileIndex);
    markReviewed.addEventListener("click", () => {
      model.diffReviewed.add(model.diffFileIndex);
      const next = diffFiles.findIndex((_, index) => model.diffSelected.has(index) && !model.diffReviewed.has(index));
      if (next >= 0) model.diffFileIndex = next;
      renderDiffInspector();
    });
    content.append(header, code, markReviewed);

    const selected = [...model.diffSelected];
    const reviewedSelected = selected.filter(index => model.diffReviewed.has(index));
    const footer = el("div", "diff-review-footer");
    const copy = el("span", "", `${reviewedSelected.length} / ${selected.length} 个已选文件完成审查`);
    const apply = el("button", "primary-button", `应用 ${selected.length} 项更改`);
    apply.type = "button";
    apply.disabled = selected.length === 0 || reviewedSelected.length !== selected.length;
    apply.addEventListener("click", () => {
      setInspector(false);
      setState(2);
    });
    footer.append(copy, apply);
    layout.append(fileList, content, footer);
    dom.inspectorBody.append(layout);
  }

  function renderIntro(intro) {
    dom.threadIntro.replaceChildren();
    if (!intro) return;
    dom.threadIntro.append(el("h2", "", intro.title));
  }

  function renderMessages(messages) {
    dom.messageList.replaceChildren();
    messages.forEach(item => {
      const article = el("article", `message ${item.role}`);
      article.append(el("div", "message-meta", item.label));
      const body = el("div", "message-body");
      item.paragraphs.forEach(paragraph => body.append(el("p", "", paragraph)));
      article.append(body);
      dom.messageList.append(article);
    });
  }

  function statusLabel(status) {
    return {
      done: "已完成",
      active: "进行中",
      pending: "等待",
      blocked: "未完成",
    }[status] || status;
  }

  function renderProgress(progress) {
    dom.workProgress.replaceChildren();
    if (!progress) return;

    const heading = el("div", "card-heading");
    const copy = el("div");
    copy.append(el("h3", "", progress.title), el("p", "", progress.summary));
    heading.append(copy);

    const current =
      progress.steps.find(item => item.status === "active" || item.status === "blocked") ||
      progress.steps.at(-1);
    const row = el("div", "compact-progress-row");
    row.append(
      el("span", `step-status ${current.status}`, statusLabel(current.status)),
      el("span", "", current.label)
    );
    if (progress.steps.length > 1) {
      const details = el("button", "text-button", `${progress.steps.length} 个步骤`);
      details.type = "button";
      details.addEventListener("click", () => setInspector(true));
      row.append(details);
    }
    dom.workProgress.append(heading, row);
  }

  function runProductAction(label) {
    const behavior = data.actionBehaviors[label];
    if (!behavior) throw new Error(`Prototype action has no behavior: ${label}`);

    if (behavior.type === "open_model") openModelPicker();
    else if (behavior.type === "open_profiles") openProfileManager();
    else if (behavior.type === "open_folder") openFolderPicker();
    else if (behavior.type === "inspect") setInspector(true);
    else if (behavior.type === "next") setState(model.stateIndex + 1);
    else if (behavior.type === "previous") setState(model.stateIndex - 1);
    else if (behavior.type === "state") setState(behavior.state);
    else if (behavior.type === "goto") {
      setJourneyById(behavior.journey);
      if (behavior.state) setState(behavior.state);
    } else if (behavior.type === "focus") dom.composerInput.focus();
    else if (behavior.type === "mode") {
      model.modeOverride = behavior.mode;
      renderComposer(currentState());
      dom.composerInput.focus();
    } else if (behavior.type === "compose_report") {
      model.modeOverride = "Work";
      renderComposer(currentState());
      dom.composerInput.value = "根据已读取证据创建 Markdown 报告，并保留来源。";
      dom.composerInput.focus();
    } else if (behavior.type === "model_then_next") {
      model.pendingModelAdvance = true;
      openModelPicker();
    } else if (behavior.type === "end_task") {
      model.resultOverride = {
        tone: "success",
        title: "任务已结束",
        summary: "安全检查点、已读取来源和停止原因均已保留；没有继续产生外部效果。",
        artifacts: [],
        actions: ["查看 Run 历史"],
      };
      render();
    } else if (behavior.type === "undo_edit") {
      model.resultOverride = {
        tone: "success",
        title: "本次变更已撤销",
        summary: "三个文件已恢复到应用前版本；撤销回执已保留。",
        artifacts: [],
        actions: ["继续修改"],
      };
      model.inspectorOverride = {
        eyebrow: "Undo receipt",
        title: "3 个文件已恢复",
        sections: [{ title: "结果", body: "工作区已恢复到应用前版本；撤销动作与恢复后的文件摘要均已记录。" }],
      };
      render();
    } else if (behavior.type === "undo_memory") {
      model.resultOverride = {
        tone: "success",
        title: "这项 Memory 已撤销",
        summary: "偏好已移除；当前 Conversation 内容没有改变，LifeModel 仍未参与。",
        artifacts: [],
        actions: ["开始不使用 Memory 的对话"],
      };
      model.inspectorOverride = {
        eyebrow: "Memory",
        title: "偏好已撤销",
        sections: [{ title: "结果", body: "Agent Memory 中不再包含这项偏好；原始 Conversation 仍作为历史来源保留。" }],
      };
      render();
    }
  }

  function actionButton(label, index) {
    const primary = index === 0;
    const button = el("button", primary ? "primary-button compact" : "secondary-button compact", label);
    button.type = "button";
    button.addEventListener("click", () => runProductAction(label));
    return button;
  }

  function renderDecision(decision) {
    dom.inlineDecision.replaceChildren();
    if (!decision) return;
    setTone(dom.inlineDecision, decision.tone);
    const heading = el("div", "decision-heading");
    const copy = el("div");
    copy.append(el("h3", "", decision.title), el("p", "", decision.body));
    heading.append(copy);

    const actions = el("div", "decision-actions");
    decision.actions.forEach((label, index) => actions.append(actionButton(label, index)));
    dom.inlineDecision.append(heading, actions);
  }

  function renderResult(result) {
    dom.resultCard.replaceChildren();
    if (!result) return;
    setTone(dom.resultCard, result.tone);
    const heading = el("div", "result-heading");
    const copy = el("div");
    copy.append(el("h3", "", result.title), el("p", "", result.summary));
    heading.append(copy);
    dom.resultCard.append(heading);

    if (result.artifacts?.length) {
      const artifacts = el("div", "artifact-list");
      result.artifacts.slice(0, 2).forEach(item => {
        const row = el("div", "artifact-row");
        row.append(el("strong", "", item.name), el("span", "", item.meta));
        artifacts.append(row);
      });
      dom.resultCard.append(artifacts);
    }

    const actions = el("div", "result-actions");
    result.actions.forEach((label, index) => actions.append(actionButton(label, index)));
    dom.resultCard.append(actions);
  }

  function renderInspector(inspector) {
    if (currentJourney().id === "edit" && model.stateIndex === 1) {
      renderDiffInspector();
      return;
    }
    if (inspector.kind === "scope") {
      const view = currentState();
      const effectiveProfile = model.profileOverride?.label || view.profile;
      const effectiveMode = model.modeOverride || view.mode;
      const scopedResources = [...new Set([...view.resources, ...model.resourceOverrides])];
      const [profileName, modelName] = effectiveProfile.split(" · ");
      const boundary = view.project
        ? `当前范围是 ${view.project} Project；读写能力以本轮资源标签和任务目标为准。`
        : scopedResources.length
          ? "当前只使用显式添加到本 Conversation 的资源；不会隐式扩大到其他本地文件。"
          : "没有 Project；本轮不会写入本地文件。";
      inspector = {
        eyebrow: "当前范围",
        title: "对话详情",
        sections: [
          { title: "边界", body: boundary },
          { title: "模型", details: [["Profile", profileName || effectiveProfile], ["Model", modelName || "未选择"], ["模式", effectiveMode]] },
        ],
      };
    }
    dom.inspectorEyebrow.textContent = inspector.eyebrow;
    dom.inspectorTitle.textContent = inspector.title;
    dom.inspectorBody.replaceChildren();

    inspector.sections.forEach(section => {
      const wrapper = el("section", "inspector-section");
      wrapper.append(el("h3", "", section.title));
      if (section.body) wrapper.append(el("p", "", section.body));
      if (section.details?.length) {
        const list = el("dl", "detail-list");
        section.details.forEach(([term, value]) => {
          const row = el("div", "detail-row");
          const detail = el("dd", /路径|文件|阶段/.test(term) ? "path" : "");
          if (/^https?:\/\//.test(value)) {
            const link = el("a", "source-link", value.replace(/^https?:\/\//, ""));
            link.href = value;
            link.target = "_blank";
            link.rel = "noreferrer";
            detail.append(link);
          } else {
            detail.textContent = value;
          }
          row.append(el("dt", "", term), detail);
          list.append(row);
        });
        wrapper.append(list);
      }
      dom.inspectorBody.append(wrapper);
    });
  }

  function renderComposer(view) {
    const searchMode = view.composerVariant === "search";
    const effectiveMode = model.modeOverride || view.mode;
    dom.modeChat.classList.toggle("active", effectiveMode === "Chat");
    dom.modeWork.classList.toggle("active", effectiveMode === "Work");
    dom.modeChat.setAttribute("aria-pressed", String(effectiveMode === "Chat"));
    dom.modeWork.setAttribute("aria-pressed", String(effectiveMode === "Work"));
    const effectiveProfile = model.profileOverride?.label || view.profile;
    dom.profileButton.textContent = effectiveProfile;
    dom.resourceChips.replaceChildren();
    const resources = [...new Set([...view.resources, ...model.resourceOverrides])];
    resources.forEach(resource => dom.resourceChips.append(el("span", "chip", resource)));
    dom.composerInput.value = view.prompt || "";
    dom.composerInput.placeholder = searchMode
      ? "搜索对话、Project 与状态…"
      : effectiveMode === "Work"
        ? "描述结果、范围和完成标准…"
        : "输入消息…";
    dom.composerInput.setAttribute("aria-label", searchMode ? "搜索历史" : "告诉 OpenLife 你想完成什么");
    dom.composerWrap.setAttribute("aria-label", searchMode ? "历史搜索" : "消息编辑器");
    dom.composerInput.rows = searchMode ? 1 : 3;
    dom.composer.classList.toggle("search-composer", searchMode);
    dom.attachButton.hidden = searchMode;
    dom.modeChat.parentElement.hidden = searchMode;
    dom.profileButton.hidden = searchMode;
    dom.sendButton.textContent = searchMode ? "搜索" : view.sendLabel;
    dom.sendButton.disabled =
      /验证中|本轮失败/.test(view.sendLabel) || effectiveProfile === "尚未选择 Profile";
  }

  function render() {
    const journey = currentJourney();
    const view = currentState();
    renderSidebar();
    dom.journeySelect.value = String(model.journeyIndex);
    dom.journeyCounter.textContent = `${journey.number} / ${data.journeys.length} · ${model.stateIndex + 1} / ${journey.states.length}`;
    dom.stateTitle.textContent = view.stateTitle;
    dom.previousState.disabled = model.stateIndex === 0;
    dom.nextState.disabled = model.stateIndex === journey.states.length - 1;
    dom.nextState.textContent = model.stateIndex === journey.states.length - 1 ? "已到最后状态" : "下一步";

    dom.contextEyebrow.textContent = view.eyebrow;
    dom.contextTitle.textContent = view.contextTitle;
    dom.contextStatus.textContent = view.contextStatus.label;
    setTone(dom.contextStatus, view.contextStatus.tone);

    const needsDetails = Boolean(view.progress || view.decision || view.result || view.contextStatus.tone === "danger");
    dom.inspectorToggle.hidden = !needsDetails;
    dom.inspectorToggle.textContent =
      currentJourney().id === "edit"
        ? "更改"
        : currentJourney().id === "web"
          ? "来源"
          : view.contextStatus.tone === "danger"
            ? "错误"
            : view.decision
              ? "查看"
              : "详情";

    renderIntro(view.intro);
    renderMessages(view.messages);
    renderProgress(view.decision || view.result ? null : view.progress);
    renderDecision(view.decision);
    renderResult(model.resultOverride || view.result);
    renderInspector(model.inspectorOverride || view.inspector);
    renderComposer(view);
    dom.thread.scrollTop = 0;
  }

  function setJourney(index) {
    model.journeyIndex = Math.max(0, Math.min(index, data.journeys.length - 1));
    model.stateIndex = 0;
    model.inspectorOpen = false;
    model.sidebarOpen = false;
    model.profileOverride = null;
    model.modeOverride = null;
    model.resultOverride = null;
    model.inspectorOverride = null;
    model.resourceOverrides = [];
    model.pendingModelAdvance = false;
    model.diffFileIndex = 0;
    model.diffSelected = new Set([0, 1, 2]);
    model.diffReviewed = new Set();
    syncPanels();
    render();
  }

  function setJourneyById(id) {
    const index = data.journeys.findIndex(journey => journey.id === id);
    if (index >= 0) setJourney(index);
  }

  function setState(index) {
    model.stateIndex = Math.max(0, Math.min(index, currentJourney().states.length - 1));
    model.resultOverride = null;
    model.inspectorOverride = null;
    render();
  }

  function setInspector(open) {
    model.inspectorOpen = open;
    if (open) model.sidebarOpen = false;
    syncPanels();
    if (open) dom.inspectorClose.focus();
    else dom.inspectorToggle.focus();
  }

  function setSidebar(open) {
    model.sidebarOpen = open;
    if (open) model.inspectorOpen = false;
    syncPanels();
  }

  function syncPanels() {
    dom.appShell.classList.toggle("inspector-open", model.inspectorOpen);
    dom.appShell.classList.toggle(
      "diff-inspector-open",
      model.inspectorOpen && currentJourney().id === "edit" && model.stateIndex === 1
    );
    dom.appShell.classList.toggle("sidebar-open", model.sidebarOpen);
    dom.inspector.setAttribute("aria-hidden", String(!model.inspectorOpen));
    dom.inspector.inert = !model.inspectorOpen;
    const sidebarHidden = window.innerWidth <= 800 && !model.sidebarOpen;
    dom.sidebar.setAttribute("aria-hidden", String(sidebarHidden));
    dom.sidebar.inert = sidebarHidden;
    dom.inspectorToggle.setAttribute("aria-expanded", String(model.inspectorOpen));
    dom.scrim.hidden = !(model.inspectorOpen || model.sidebarOpen) || window.innerWidth > 1180;
  }

  function setPrototypeControls(open) {
    model.controlsVisible = open;
    dom.controls.hidden = !open;
    dom.controlsToggle.setAttribute("aria-expanded", String(open));
    if (open) dom.journeySelect.focus();
  }

  function bindEvents() {
    dom.journeySelect.addEventListener("change", event => setJourney(Number(event.target.value)));
    dom.previousState.addEventListener("click", () => setState(model.stateIndex - 1));
    dom.nextState.addEventListener("click", () => setState(model.stateIndex + 1));
    dom.sendButton.addEventListener("click", () => {
      if (model.stateIndex < currentJourney().states.length - 1) setState(model.stateIndex + 1);
    });
    dom.modeChat.addEventListener("click", () => {
      model.modeOverride = "Chat";
      renderComposer(currentState());
    });
    dom.modeWork.addEventListener("click", () => {
      model.modeOverride = "Work";
      renderComposer(currentState());
    });
    dom.settingsButton.addEventListener("click", openProfileManager);
    dom.attachButton.addEventListener("click", openResourcePicker);
    dom.profileButton.setAttribute("aria-haspopup", "dialog");
    dom.profileButton.addEventListener("click", openModelPicker);
    dom.inspectorToggle.addEventListener("click", () => setInspector(!model.inspectorOpen));
    dom.inspectorClose.addEventListener("click", () => setInspector(false));
    dom.sidebarToggle.addEventListener("click", () => setSidebar(true));
    dom.scrim.addEventListener("click", () => {
      model.inspectorOpen = false;
      model.sidebarOpen = false;
      syncPanels();
    });
    dom.controlsToggle.addEventListener("click", () => {
      setPrototypeControls(!model.controlsVisible);
    });
    dom.controlsClose.addEventListener("click", () => {
      setPrototypeControls(false);
      dom.composerInput.focus();
    });
    dom.modelPickerClose.addEventListener("click", closeModelPicker);
    dom.modelPickerCancel.addEventListener("click", closeModelPicker);
    dom.profileOptions.addEventListener("click", event => {
      const row = event.target.closest("[data-profile-id]");
      if (!row) return;
      model.selectedProfileId = row.dataset.profileId;
      model.selectedModel = profiles.find(profile => profile.id === model.selectedProfileId).models[0];
      renderModelPicker();
    });
    dom.modelSelect.addEventListener("change", event => {
      model.selectedModel = event.target.value;
      renderModelPicker();
    });
    dom.reasoningOptions.addEventListener("click", event => {
      const button = event.target.closest("[data-reasoning]");
      if (!button) return;
      model.reasoning = button.dataset.reasoning;
      renderModelPicker();
    });
    dom.useModel.addEventListener("click", () => {
      const profile = profiles.find(item => item.id === model.selectedProfileId);
      model.profileOverride = {
        profileId: profile.id,
        model: model.selectedModel,
        label: `${profile.name} · ${model.selectedModel}`,
      };
      dom.modelPicker.close();
      if (model.pendingModelAdvance) {
        model.pendingModelAdvance = false;
        setState(model.stateIndex + 1);
      } else if (currentJourney().id === "onboarding" && model.stateIndex === 0) setState(1);
      else render();
      dom.composerInput.focus();
    });
    dom.manageProfiles.addEventListener("click", () => {
      openProfileManager();
    });
    dom.profileManagerClose.addEventListener("click", closeProfileManager);
    dom.addProfile.addEventListener("click", () => {
      dom.profileForm.hidden = false;
      dom.profileName.focus();
    });
    dom.verifyProfile.addEventListener("click", () => {
      const name = dom.profileName.value.trim();
      const endpoint = dom.profileEndpoint.value.trim();
      if (!name || !endpoint) {
        dom.profileValidation.textContent = "请填写 Profile name 与 endpoint。";
        return;
      }
      dom.profileValidation.textContent = "已发现 2 个模型；连接、Chat 与 Work 结构化步骤已验证（原型模拟）。";
      if (!profiles.some(profile => profile.name === name)) {
        profiles.push({
          id: `profile-${profiles.length + 1}`,
          name,
          meta: "自定义端点 · 已验证",
          tone: "success",
          models: ["team-work-model", "team-chat-model"],
          summary: `通过 ${dom.providerType.options[dom.providerType.selectedIndex].text} 连接；请求会发送到 ${endpoint}。`,
        });
      }
      dom.profileCredential.value = "";
      renderProfileManager();
    });
    dom.resourcePickerClose.addEventListener("click", closeResourcePicker);
    dom.addLocalFile.addEventListener("click", () => addConversationResource("customer-notes.pdf · 只读"));
    dom.addProjectFolder.addEventListener("click", () => {
      closeResourcePicker();
      openFolderPicker();
    });
    dom.addResourceUrl.addEventListener("click", () => {
      const url = dom.resourceUrl.value.trim();
      if (!/^https?:\/\//.test(url)) {
        dom.resourceNotice.textContent = "请输入完整的 http 或 https URL。";
        return;
      }
      addConversationResource(`${url.replace(/^https?:\/\//, "")} · 公开`);
    });
    dom.folderPickerClose.addEventListener("click", closeFolderPicker);
    dom.folderPickerCancel.addEventListener("click", closeFolderPicker);
    dom.folderOptions.addEventListener("click", event => {
      const row = event.target.closest("[data-folder-id]");
      if (!row) return;
      model.selectedFolderId = row.dataset.folderId;
      model.folderPermissionGranted = false;
      renderFolderPicker();
    });
    dom.browseFolder.addEventListener("click", () => {
      model.selectedFolderId = "open-life";
      model.folderPermissionGranted = true;
      renderFolderPicker();
      dom.folderNotice.textContent = "系统选择器已返回：/Users/tw/Desktop/open-life（原型模拟，不读取文件）。";
    });
    dom.openSelectedFolder.addEventListener("click", () => {
      const folder = folders.find(item => item.id === model.selectedFolderId);
      if (!folder || folder.tone === "danger") return;
      if (folder.tone === "warning") model.folderPermissionGranted = true;
      dom.folderPicker.close();
      setJourneyById("project");
      setState(1);
    });
    document.addEventListener("click", event => {
      if (event.target.closest("[data-open-folder]")) {
        openFolderPicker();
        return;
      }
      const target = event.target.closest("[data-journey]");
      if (target) setJourneyById(target.dataset.journey);
    });
    document.addEventListener("keydown", event => {
      if (event.key === "Escape") {
        if (model.inspectorOpen) setInspector(false);
        else if (model.sidebarOpen) setSidebar(false);
      }
      if (event.altKey && event.key === "ArrowRight") setState(model.stateIndex + 1);
      if (event.altKey && event.key === "ArrowLeft") setState(model.stateIndex - 1);
      if (event.altKey && event.key.toLocaleLowerCase("zh-CN") === "p") {
        event.preventDefault();
        setPrototypeControls(!model.controlsVisible);
      }
    });
    window.addEventListener("resize", syncPanels);
  }

  renderJourneyOptions();
  renderSidebar();
  bindEvents();
  syncPanels();
  render();
})();
