(() => {
  const evidence = (id, label, source, sensitivity = "local_private") => ({
    id,
    label,
    source,
    sensitivity,
  });

  const unknownBoundary = (evidenceRefs, blockedReason) => ({
    routeType: "unknown",
    externalTransmission: "unknown",
    providerLabel: "尚未确认",
    modelLabel: "尚未确认",
    privacyLabel: "模型与传输边界尚未确认",
    risk: "unknown",
    localOnlyRequired: false,
    blockedReason,
    evidenceRefs,
  });

  const possibleBoundary = (evidenceRefs, blockedReason) => ({
    routeType: "auto",
    externalTransmission: "possible",
    providerLabel: "云端供应商尚未确认",
    modelLabel: "自动选择",
    privacyLabel: "可能发生外部传输",
    risk: "medium",
    localOnlyRequired: false,
    blockedReason,
    evidenceRefs,
  });

  const rawJsonAction = (id, targetRef) => ({
    id,
    label: "查看 fixture JSON",
    kind: "raw_json",
    enabled: true,
    developerOnly: true,
    targetRef,
    sourceRef: "ViewModelEnvelope.actions.debugOnly",
    fixtureBehavior: { type: "dialog", payload: "state" },
  });

  const todayReadyEvidence = [
    evidence("fixture:today:ready-2026-07-10", "今日计划快照", "backend-readmodel"),
    evidence("fixture:review:deep-work-pending", "深度工作偏好建议", "review"),
    evidence(
      "fixture:provider:today-boundary-unknown",
      "模型与传输边界尚未确认",
      "provider",
      "sensitive",
    ),
  ];

  const todayStaleEvidence = [
    evidence("fixture:today:stale-2026-07-09", "昨日计划快照", "backend-readmodel"),
    evidence("fixture:review:status-unavailable", "审核状态无法刷新", "review"),
    evidence(
      "fixture:provider:route-missing",
      "缺少运行时路由依据",
      "provider",
      "sensitive",
    ),
  ];

  const workspaceEvidence = [
    evidence("fixture:task:hangzhou-trip", "杭州周末行程整理任务", "task"),
    evidence(
      "fixture:review:travel-folder-permission",
      "旅行资料文件夹访问请求",
      "review",
      "sensitive",
    ),
    evidence(
      "fixture:provider:workspace-transmission-possible",
      "任务可能使用外部模型",
      "provider",
      "sensitive",
    ),
  ];

  const reviewPendingEvidence = [
    evidence("fixture:proposal:deep-work-preference", "深度工作偏好建议原始提案", "review"),
    evidence("fixture:audit:planning-pattern", "三次晨间计划记录", "audit"),
    evidence(
      "fixture:lifemodel:preference-target",
      "长期偏好影响对象",
      "lifemodel",
      "sensitive",
    ),
  ];

  const reviewApprovedEvidence = [
    evidence("fixture:review:deep-work-approved", "用户批准决定", "review"),
    evidence(
      "fixture:lifemodel:application-missing",
      "尚未取得长期状态应用结果",
      "lifemodel",
      "sensitive",
    ),
    evidence("fixture:audit:decision-approved", "批准决定审计记录", "audit"),
  ];

  const lifeModelEvidence = [
    evidence(
      "fixture:lifemodel:compatibility-view",
      "当前兼容视图",
      "lifemodel",
      "sensitive",
    ),
    evidence("fixture:audit:calendar-buffer-source", "会议缓冲偏好来源", "audit"),
    evidence("fixture:review:deep-work-pending-link", "待决定偏好建议", "review"),
  ];

  const settingsEvidence = [
    evidence(
      "fixture:settings:local-model-configured",
      "本地模型配置样例",
      "settings",
      "sensitive",
    ),
    evidence(
      "fixture:provider:cloud-transmission-possible",
      "云端传输可能发生",
      "provider",
      "sensitive",
    ),
    evidence(
      "fixture:audit:no-runtime-route-proof",
      "没有当前运行时路由证明",
      "audit",
      "redacted",
    ),
  ];

  window.OPENLIFE_MOCKUP_NAV = [
    {
      key: "today",
      label: "今日",
      icon: "calendar",
      placement: "primary",
      defaultStateId: "today-ready-pending-review",
      mobilePrimary: true,
    },
    {
      key: "workspace",
      label: "工作区",
      icon: "workspace",
      placement: "primary",
      defaultStateId: "workspace-waiting-permission",
      mobilePrimary: true,
    },
    {
      key: "tasks",
      label: "任务",
      icon: "tasks",
      placement: "primary",
      unavailable: {
        title: "任务页尚未开放",
        detail: "独立任务信息模型尚未成熟；当前执行仍从工作区进入。",
      },
    },
    {
      key: "review",
      label: "审核中心",
      icon: "review",
      placement: "primary",
      defaultStateId: "review-pending-decision",
      mobilePrimary: true,
    },
    {
      key: "lifemodel",
      label: "LifeModel",
      icon: "lifemodel",
      placement: "primary",
      defaultStateId: "lifemodel-limited-compat",
      mobilePrimary: true,
    },
    {
      key: "settings",
      label: "设置",
      icon: "settings",
      placement: "utility",
      defaultStateId: "settings-provider-privacy-unknown",
    },
  ];

  window.OPENLIFE_MOCKUP_STATES = [
    {
      id: "today-ready-pending-review",
      navKey: "today",
      shortLabel: "今日：计划可用，有一项待决定建议",
      envelope: {
        status: "ready",
        lastUpdatedAt: "fixture-only",
        source: "backend-readmodel",
      },
      surface: { title: "今日", eyebrow: "7 月 10 日，星期五" },
      primaryStatus: {
        label: "今日计划可用",
        tone: "is-success",
        sourceRef: "ViewModelEnvelope.status + TodayViewModel.dailyStateSummary.readiness",
      },
      privacyBoundary: unknownBoundary(
        [todayReadyEvidence[2]],
        "No runtime route evidence proves that external transmission was not sent.",
      ),
      inspectorSummary: {
        happened: "今日计划可以查看，一项长期偏好建议等待你的决定。",
        risk: "建议尚未改变长期状态；模型与传输边界仍未确认。",
        next: "先查看建议内容，再决定批准、拒绝、修改或稍后处理。",
      },
      goal: {
        label: "当前重点",
        title: "上午完成客户方案初稿",
        summary: "保留 09:00–11:00 的连续时间，午前完成结构和关键数据核对。",
        sourceRef: "TodayViewModel.primaryDailyGoal",
      },
      blocker: {
        label: "需要你决定",
        title: "是否把上午深度工作设为长期偏好",
        body: "建议只在审核中心等待，不会因为出现在今日页就改变长期状态。",
        tone: "is-warning",
        sourceRef: "TodayViewModel.blockers[waiting_review] + pendingReviewCount",
      },
      metrics: [
        { value: "2", label: "今日重点", sourceRef: "layout-only: daily focus count fixture" },
        { value: "11:30", label: "下次提醒", sourceRef: "layout-only: daily schedule fixture" },
        { value: "0", label: "已授权外部动作", sourceRef: "layout-only: external action count fixture" },
      ],
      actionPrompt: "选择下一步",
      actions: {
        primary: [
          {
            id: "today:open-pending-review",
            label: "查看待决定建议",
            kind: "open",
            enabled: true,
            targetRef: "review_item:fixture:review:deep-work-pending",
            sourceRef: "ViewModelEnvelope.actions.primary",
            fixtureBehavior: { type: "navigate", stateId: "review-pending-decision" },
          },
          {
            id: "today:inspect-plan-basis",
            label: "查看今日依据",
            kind: "inspect",
            enabled: true,
            targetRef: "evidence:fixture:today:ready-2026-07-10",
            sourceRef: "ViewModelEnvelope.actions.primary",
            fixtureBehavior: {
              type: "open_inspector",
              evidenceId: "fixture:today:ready-2026-07-10",
            },
          },
        ],
        review: [],
        debugOnly: [rawJsonAction("today:raw-json", "fixture:today:ready-2026-07-10")],
      },
      sections: [
        {
          label: "今日安排",
          title: "接下来怎么推进",
          rows: [
            {
              title: "完成方案结构",
              body: "先写清问题、方案和验证数据，暂不发送给外部收件人。",
              meta: "进行中",
              sourceRef: "TodayViewModel.primaryDailyGoal",
            },
            {
              title: "午后复诊",
              body: "14:20 出发，预留 30 分钟路程。",
              meta: "14:20",
              sourceRef: "layout-only: daily schedule fixture",
            },
          ],
        },
        {
          label: "边界",
          title: "今天不会自动发生的事",
          rows: [
            {
              title: "不会自动发送方案",
              body: "草稿完成后仍需你明确确认收件人与内容。",
              meta: "需确认",
              sourceRef: "TodayViewModel.safeMode",
            },
            {
              title: "不会自动改变长期偏好",
              body: "待决定建议仍与当前 LifeModel 分离。",
              meta: "未改变",
              sourceRef: "ReviewItem.status + ReviewItem.materializationStatus",
            },
          ],
        },
      ],
      evidenceRefs: todayReadyEvidence,
      limitations: [
        "今日数据为固定 fixture，不是实时计划。",
        "模型与传输边界未知，因此不能显示绿色本地确定态。",
      ],
      fixtureId: "fixture.today.pending_review.v3",
    },
    {
      id: "today-stale-unknown",
      navKey: "today",
      shortLabel: "今日：快照陈旧，保持关闭",
      envelope: {
        status: "stale",
        lastUpdatedAt: null,
        source: "backend-readmodel",
      },
      surface: { title: "今日", eyebrow: "当前快照不可确认" },
      primaryStatus: {
        label: "计划已陈旧",
        tone: "is-warning",
        sourceRef: "ViewModelEnvelope.status",
      },
      privacyBoundary: unknownBoundary(
        [todayStaleEvidence[2]],
        "Runtime route and transmission evidence are unavailable.",
      ),
      inspectorSummary: {
        happened: "只能读取昨日快照，今天的计划与审核状态无法确认。",
        risk: "继续执行可能使用过期目标或遗漏新的审核阻塞。",
        next: "查看缺失依据；等待可信刷新后再生成今日结论。",
      },
      goal: {
        label: "当前重点",
        title: "先恢复可信快照",
        summary: "昨日内容可以作为参考，但不能当作今天仍然成立的计划。",
        sourceRef: "TodayViewModel.dailyStateSummary + ViewModelEnvelope.status",
      },
      blocker: {
        label: "安全模式",
        title: "风险动作保持关闭",
        body: "刷新、外部调用与持久写入都不能由这个静态页面代替执行。",
        tone: "is-safe-mode",
        sourceRef: "TodayViewModel.safeMode + ViewModelEnvelope.warnings",
      },
      metrics: [
        { value: "2", label: "可查看历史项", sourceRef: "layout-only: stale snapshot fixture" },
        { value: "0", label: "可执行动作", sourceRef: "ViewModelEnvelope.actions.primary[].enabled" },
        { value: "昨日", label: "快照日期", sourceRef: "ViewModelEnvelope.lastUpdatedAt" },
      ],
      actionPrompt: "处理不可用状态",
      actions: {
        primary: [
          {
            id: "today:refresh-stale",
            label: "刷新今日计划",
            kind: "refresh",
            enabled: false,
            disabledReason: "仅样式：静态样例没有连接刷新命令。",
            targetRef: "readmodel:today",
            sourceRef: "ViewModelEnvelope.actions.primary",
          },
          {
            id: "today:inspect-stale-evidence",
            label: "查看缺失依据",
            kind: "inspect",
            enabled: true,
            targetRef: "evidence:fixture:today:stale-2026-07-09",
            sourceRef: "ViewModelEnvelope.actions.primary",
            fixtureBehavior: {
              type: "open_inspector",
              evidenceId: "fixture:today:stale-2026-07-09",
            },
          },
        ],
        review: [],
        debugOnly: [rawJsonAction("today-stale:raw-json", "fixture:today:stale-2026-07-09")],
      },
      sections: [
        {
          label: "历史参考",
          title: "仍可安全查看",
          rows: [
            {
              title: "昨日的客户方案任务",
              body: "保留为历史记录，不自动延续为今天的目标。",
              meta: "只读",
              sourceRef: "ViewModelEnvelope.status",
            },
            {
              title: "昨日的复诊提醒",
              body: "时间可能已经变化，需要重新确认。",
              meta: "待确认",
              sourceRef: "ViewModelEnvelope.lastUpdatedAt",
            },
          ],
        },
      ],
      evidenceRefs: todayStaleEvidence,
      limitations: [
        "空白不能解释为今天没有任务。",
        "页面不会从诊断片段重建今日状态。",
      ],
      fixtureId: "fixture.today.stale_unknown.v3",
    },
    {
      id: "workspace-waiting-permission",
      navKey: "workspace",
      shortLabel: "工作区：等待文件访问决定",
      envelope: {
        status: "ready",
        lastUpdatedAt: "fixture-only",
        source: "backend-readmodel",
      },
      layout: "workspace_timeline",
      inspectorMode: "on_demand",
      inspectorHeading: "访问范围",
      surface: { title: "工作区", eyebrow: "杭州周末行程" },
      primaryStatus: {
        label: "等待访问决定",
        tone: "is-warning",
        sourceRef: "TasksViewModel.items[].lifecycleStatus",
      },
      privacyBoundary: possibleBoundary(
        [workspaceEvidence[2]],
        "Provider route and one-time permission semantics are not proven.",
      ),
      inspectorSummary: {
        happened: "任务请求读取一个旅行资料文件夹，以整理日期和预订信息。",
        risk: "请求没有可靠表达一次性时效、撤销方式或是否会外传。",
        next: "可以拒绝或查看范围；一次性允许保持禁用，直到契约补齐。",
      },
      goal: {
        label: "当前任务",
        title: "整理杭州周末行程",
        summary: "把已有票据和预订整理成一份可复查的行程草稿。",
        sourceRef: "WorkspaceViewModel.activeTaskRef + TasksViewModel.items[].title",
      },
      blocker: {
        label: "访问边界不完整",
        title: "当前请求不能安全表达“仅允许本次”",
        body: "在时效、撤销和传输边界明确前，不继续读取文件。",
        tone: "is-warning",
        sourceRef: "PROPOSED ReviewDecisionContext.permissionScope",
      },
      metrics: [],
      timeline: [
        {
          id: "workspace:event:outline-ready",
          status: "done",
          label: "已完成",
          title: "已建立行程结构",
          body: "城市、日期和交通段落已经准备好。",
          meta: "本地",
          sourceRef: "WorkspaceViewModel.timeline[]",
        },
        {
          id: "workspace:event:permission-request",
          status: "waiting",
          label: "需要你决定",
          title: "读取“旅行/杭州周末”",
          body: "核对车票、酒店和活动日期，再继续生成行程草稿。",
          meta: "尚未执行",
          sourceRef: "WorkspaceViewModel.timeline[] + ReviewItem.status",
        },
        {
          id: "workspace:event:draft-pending",
          status: "pending",
          label: "下一步",
          title: "生成可复查的行程草稿",
          body: "访问决定明确后才会继续。",
          meta: "等待",
          sourceRef: "WorkspaceViewModel.timeline[] + TasksViewModel.items[].lifecycleStatus",
        },
      ],
      permissionContext: {
        contractStatus: "PROPOSED_REVIEW_PROJECTION",
        title: "读取旅行资料",
        purpose: "提取车票、酒店和活动日期，用于生成行程草稿。",
        tool: "本地文件读取",
        capability: "filesystem.read",
        target: "~/Documents/旅行/杭州周末/",
        dataScope: "该文件夹中的 4 份 PDF；不包含上级目录。",
        transmission: "未知；没有运行时路由依据证明内容不会外传。",
        summaryItems: [
          {
            label: "4 份 PDF",
            tone: "is-neutral",
            sourceRef: "PROPOSED PermissionDecisionContext.dataScopeSummary",
          },
          {
            label: "仅此文件夹",
            tone: "is-neutral",
            sourceRef: "PROPOSED PermissionDecisionContext.targetLabel",
          },
          {
            label: "外传未知",
            tone: "is-warning",
            sourceRef: "ProviderPrivacyBoundarySummary.externalTransmission",
          },
        ],
        duration: "请求中没有说明；不能按一次性授权处理。",
        revocation: "请求中没有说明；无法确认任务结束后自动失效。",
        currentPolicy: "allow_until_revoked",
        sourceRef: "AgentProposal.after.canonical_scope + blocked_action (not projected by ReviewItem)",
      },
      actionPrompt: "决定这次访问",
      actions: {
        primary: [
          {
            id: "workspace:inspect-task",
            label: "查看任务依据",
            kind: "inspect",
            enabled: true,
            targetRef: "task:fixture:task:hangzhou-trip",
            sourceRef: "ViewModelEnvelope.actions.primary",
            fixtureBehavior: {
              type: "open_inspector",
              evidenceId: "fixture:task:hangzhou-trip",
            },
          },
        ],
        review: [
          {
            id: "workspace:reject-permission",
            label: "拒绝",
            kind: "reject",
            effect: "decision_only",
            enabled: true,
            requiresConfirmation: false,
            targetReviewItemId: "fixture:review:travel-folder-permission",
            sourceRef: "ViewModelEnvelope.actions.review",
            fixtureBehavior: {
              type: "dialog",
              title: "拒绝这次访问",
              message: "静态结果：任务会保持阻塞，不会读取该文件夹。",
            },
          },
          {
            id: "workspace:view-permission-scope",
            label: "查看访问范围",
            kind: "view_evidence",
            effect: "evidence_only",
            enabled: true,
            requiresConfirmation: false,
            targetReviewItemId: "fixture:review:travel-folder-permission",
            sourceRef: "ViewModelEnvelope.actions.review + PROPOSED permission projection",
            fixtureBehavior: { type: "open_inspector", sectionId: "permissionScopePanel" },
          },
          {
            id: "workspace:allow-once",
            label: "仅允许本次",
            kind: "approve",
            effect: "decision_only",
            enabled: false,
            disabledReason: "当前契约没有一次性时效和撤销语义。",
            requiresConfirmation: true,
            targetReviewItemId: "fixture:review:travel-folder-permission",
            expectedMaterializationStatusAfterDispatch: "unknown",
            sourceRef: "PROPOSED ReviewAction over current approve action",
          },
        ],
        debugOnly: [rawJsonAction("workspace:raw-json", "fixture:task:hangzhou-trip")],
      },
      sections: [],
      evidenceRefs: workspaceEvidence,
      limitations: [
        "当前 ReviewItem 未投影权限时效、撤销方式或完整 canonical_scope。",
        "一次性授权是目标设计，不是当前后端已支持能力。",
      ],
      fixtureId: "fixture.workspace.permission_scope_gap.v3",
    },
    {
      id: "review-pending-decision",
      navKey: "review",
      shortLabel: "审核：深度工作偏好等待决定",
      envelope: {
        status: "ready",
        lastUpdatedAt: "fixture-only",
        source: "backend-readmodel",
      },
      surface: { title: "审核中心", eyebrow: "1 项建议等待决定" },
      primaryStatus: {
        label: "等待你的决定",
        tone: "is-warning",
        sourceRef: "ReviewItem.status",
      },
      privacyBoundary: unknownBoundary(
        [],
        "This review fixture does not include current provider route evidence.",
      ),
      inspectorSummary: {
        happened: "OpenLife 建议把工作日上午设为优先深度工作的长期偏好。",
        risk: "批准会影响未来排程建议，但不会直接修改日历或发送消息。",
        next: "比较当前与建议内容，再选择拒绝、稍后、修改或批准变更。",
      },
      goal: {
        label: "建议变更",
        title: "工作日上午优先安排深度工作",
        summary: "这是一项待决定建议，不代表已经批准，也没有进入长期状态。",
        sourceRef: "ReviewCenterViewModel.items[0] + PROPOSED ReviewDecisionContext",
      },
      blocker: {
        label: "影响范围",
        title: "将改变未来的排程建议",
        body: "不会自动移动已有日程；之后生成的新计划会优先保留上午连续时间。",
        tone: "is-warning",
        sourceRef: "PROPOSED ReviewDecisionContext.impactSummary",
      },
      metrics: [
        { value: "3 次", label: "观察依据", sourceRef: "PROPOSED ReviewDecisionContext.sourceCount" },
        { value: "低", label: "建议风险", sourceRef: "ReviewItem.risk" },
        { value: "7 天", label: "剩余有效期", sourceRef: "ReviewItem.expiresAt" },
      ],
      reviewContext: {
        contractStatus: "PROPOSED_REVIEW_PROJECTION",
        changeSummary: "以后生成工作日计划时，优先保留 09:00–11:00 的连续专注时间。",
        before: "工作日上午没有固定偏好。",
        after: "工作日 09:00–11:00 优先安排深度工作。",
        reason: "最近三次晨间计划中，你都主动把需要专注的工作移到上午。",
        source: "晨间计划记录（3 次）",
        risk: "低风险；影响未来建议，不直接修改现有日历。",
        impact: "LifeModel 的时间偏好，以及后续每日计划排序。",
        expires: "7 天后过期；过期后需要重新提出。",
        target: "LifeModel 的时间偏好",
        sourceRef: "AgentProposal.before/after/reason/affected_path/expires_at (not projected by ReviewItem)",
      },
      stickyDecisionActions: true,
      actionPrompt: "你想怎样处理",
      actions: {
        primary: [],
        review: [
          {
            id: "review:reject-deep-work",
            label: "拒绝",
            kind: "reject",
            effect: "decision_only",
            enabled: true,
            requiresConfirmation: false,
            targetReviewItemId: "fixture:review:deep-work-pending",
            sourceRef: "ReviewItem.allowedActions",
            fixtureBehavior: {
              type: "dialog",
              title: "拒绝这项建议",
              message: "静态结果：长期偏好保持不变，建议会结束。",
            },
          },
          {
            id: "review:later-deep-work",
            label: "稍后处理",
            kind: "later",
            effect: "decision_only",
            enabled: true,
            requiresConfirmation: false,
            targetReviewItemId: "fixture:review:deep-work-pending",
            sourceRef: "ReviewItem.allowedActions",
            fixtureBehavior: {
              type: "dialog",
              title: "稍后处理",
              message: "静态结果：建议保持待决定，不会改变长期状态。",
            },
          },
          {
            id: "review:edit-deep-work",
            label: "修改",
            kind: "edit",
            effect: "decision_only",
            enabled: true,
            requiresConfirmation: false,
            targetReviewItemId: "fixture:review:deep-work-pending",
            sourceRef: "ReviewItem.allowedActions",
            fixtureBehavior: {
              type: "dialog",
              title: "修改建议",
              message: "静态结果：这里验证编辑入口；生产实现需要提交修改后的 after 值。",
            },
          },
          {
            id: "review:approve-deep-work",
            label: "批准变更",
            kind: "approve",
            effect: "decision_only",
            enabled: true,
            requiresConfirmation: true,
            targetReviewItemId: "fixture:review:deep-work-pending",
            expectedMaterializationStatusAfterDispatch: "unknown",
            sourceRef: "ReviewItem.allowedActions",
            fixtureBehavior: {
              type: "confirm_transition",
              stateId: "review-approved-not-materialized",
              title: "批准这项变更？",
              message: "批准只记录你的决定。长期状态只有在后端返回应用结果后才会更新。",
              confirmLabel: "确认批准",
            },
          },
        ],
        debugOnly: [rawJsonAction("review-pending:raw-json", "fixture:proposal:deep-work-preference")],
      },
      sections: [
        {
          label: "来源记录",
          title: "为什么出现这项建议",
          rows: [
            {
              title: "三次晨间计划",
              body: "每次都把写作或方案工作移动到上午连续时段。",
              meta: "3 次",
              sourceRef: "AgentProposal.source_detail",
            },
            {
              title: "不影响已有日历",
              body: "建议只改变未来排序，不自动移动现有事件。",
              meta: "无直接写入",
              sourceRef: "AgentProposal.affected_path + after",
            },
          ],
        },
      ],
      evidenceRefs: reviewPendingEvidence,
      limitations: [
        "before、after、原因和影响来自 AgentProposal 静态映射；当前 ReviewItem 未投影这些字段。",
        "批准后的当前契约只保证决定状态变化，不能保证立即进入 applying。",
      ],
      fixtureId: "fixture.review.pending_decision.v1",
    },
    {
      id: "review-approved-not-materialized",
      navKey: "review",
      shortLabel: "审核：已批准，尚未应用",
      envelope: {
        status: "ready",
        lastUpdatedAt: "fixture-only",
        source: "backend-readmodel",
      },
      surface: { title: "审核中心", eyebrow: "决定已记录" },
      primaryStatus: {
        label: "已批准，尚未应用",
        tone: "is-warning",
        sourceRef: "ReviewItem.status + ReviewItem.materializationStatus",
      },
      privacyBoundary: unknownBoundary(
        [],
        "This review fixture does not include current provider route evidence.",
      ),
      inspectorSummary: {
        happened: "用户已经批准深度工作偏好建议。",
        risk: "长期状态尚未返回应用结果，当前 LifeModel 不能显示新偏好。",
        next: "等待后端应用能力与刷新后的 read model；不要把批准显示为完成。",
      },
      goal: {
        label: "当前结果",
        title: "批准决定已经记录",
        summary: "长期状态仍按原偏好显示，直到取得可信的应用结果。",
        sourceRef: "ReviewCenterViewModel.items[0]",
      },
      blocker: {
        label: "尚未应用",
        title: "批准不等于长期状态已经改变",
        body: "系统尚未返回应用结果，因此不会显示“已完成”。",
        tone: "is-warning",
        sourceRef: "ReviewItem.materializationStatus",
      },
      metrics: [
        { value: "1", label: "已记录决定", sourceRef: "ReviewCenterViewModel.summary.byStatus.approved" },
        { value: "0", label: "已应用变更", sourceRef: "ReviewCenterViewModel.summary.byMaterializationStatus.applied" },
        { value: "0", label: "失败记录", sourceRef: "ReviewCenterViewModel.summary.byMaterializationStatus.failed" },
      ],
      actionPrompt: "查看结果或等待应用",
      actions: {
        primary: [
          {
            id: "review:inspect-application-status",
            label: "查看应用依据",
            kind: "inspect",
            enabled: true,
            targetRef: "evidence:fixture:lifemodel:application-missing",
            sourceRef: "ViewModelEnvelope.actions.primary",
            fixtureBehavior: {
              type: "open_inspector",
              evidenceId: "fixture:lifemodel:application-missing",
            },
          },
        ],
        review: [
          {
            id: "review:request-apply",
            label: "应用变更",
            kind: "apply",
            effect: "materialization_request",
            enabled: false,
            disabledReason: "当前后端没有可用的应用请求命令。",
            requiresConfirmation: true,
            targetReviewItemId: "fixture:review:deep-work-approved",
            sourceRef: "ReviewItem.allowedActions",
            contractGapRef: "REVIEW_MATERIALIZATION_DISPATCH_REQUIRED",
          },
        ],
        debugOnly: [rawJsonAction("review-approved:raw-json", "fixture:review:deep-work-approved")],
      },
      sections: [
        {
          label: "决策记录",
          title: "已经确定的内容",
          rows: [
            {
              title: "用户决定",
              body: "批准工作日上午优先深度工作的建议。",
              meta: "已批准",
              sourceRef: "ReviewItem.status",
            },
            {
              title: "当前长期偏好",
              body: "仍保持批准前的内容，没有应用依据时不会提前更新。",
              meta: "未改变",
              sourceRef: "ReviewItem.materializationStatus",
            },
          ],
        },
      ],
      evidenceRefs: reviewApprovedEvidence,
      limitations: [
        "当前批准动作的 expectedMaterializationStatusAfterDispatch 为 unknown。",
        "目标流程需要后端先返回 applying，再由刷新后的 read model 证明 applied。",
      ],
      fixtureId: "fixture.review.approved_not_materialized.v3",
    },
    {
      id: "lifemodel-limited-compat",
      navKey: "lifemodel",
      shortLabel: "LifeModel：当前兼容视图受限",
      envelope: {
        status: "ready",
        lastUpdatedAt: "fixture-only",
        source: "backend-readmodel",
      },
      surface: { title: "LifeModel", eyebrow: "长期理解" },
      primaryStatus: {
        label: "当前视图受限",
        tone: "is-warning",
        sourceRef: "LifeModelViewModel.truthMode + contractLimitations",
      },
      privacyBoundary: unknownBoundary(
        [lifeModelEvidence[0]],
        "The compatibility view does not prove provider routing or transmission state.",
      ),
      inspectorSummary: {
        happened: "当前只能展示有限兼容视图和已有来源引用。",
        risk: "待决定或尚未应用的建议不能进入当前长期理解。",
        next: "查看来源；需要改变偏好时先进入审核中心。",
      },
      goal: {
        label: "当前理解",
        title: "会议前预留 15 分钟准备时间",
        summary: "这条偏好有现有来源；上午深度工作建议仍在审核流程外。",
        sourceRef: "LifeModelViewModel.currentViewSummary + truthMode",
      },
      blocker: {
        label: "兼容边界",
        title: "待决定建议不会混入当前视图",
        body: "只有刷新后的长期状态明确包含变更，才会作为当前理解展示。",
        tone: "is-warning",
        sourceRef: "LifeModelViewModel.contractLimitations",
      },
      metrics: [
        { value: "3", label: "来源引用", sourceRef: "LifeModelViewModel.provenanceRefs.length" },
        { value: "1", label: "待决定建议", sourceRef: "LifeModelViewModel.pendingUpdateCounts" },
        { value: "0", label: "新应用变更", sourceRef: "LifeModelViewModel.materializedChanges.length" },
      ],
      actionPrompt: "查看当前理解或待决定建议",
      actions: {
        primary: [
          {
            id: "lifemodel:inspect-current-view",
            label: "查看来源",
            kind: "inspect",
            enabled: true,
            targetRef: "lifemodel:fixture:lifemodel:compatibility-view",
            sourceRef: "ViewModelEnvelope.actions.primary",
            fixtureBehavior: {
              type: "open_inspector",
              evidenceId: "fixture:lifemodel:compatibility-view",
            },
          },
          {
            id: "lifemodel:open-pending-review",
            label: "查看待决定建议",
            kind: "open",
            enabled: true,
            targetRef: "review_item:fixture:review:deep-work-pending",
            sourceRef: "ViewModelEnvelope.actions.primary",
            fixtureBehavior: { type: "navigate", stateId: "review-pending-decision" },
          },
        ],
        review: [],
        debugOnly: [rawJsonAction("lifemodel:raw-json", "fixture:lifemodel:compatibility-view")],
      },
      sections: [
        {
          label: "已有偏好",
          title: "当前可追溯的理解",
          rows: [
            {
              title: "会议准备缓冲",
              body: "安排会议时优先预留 15 分钟准备时间。",
              meta: "当前",
              sourceRef: "LifeModelViewModel.currentViewSummary",
            },
            {
              title: "来源记录",
              body: "来自两次手动调整和一次明确反馈。",
              meta: "3 条",
              sourceRef: "LifeModelViewModel.provenanceRefs",
            },
          ],
        },
      ],
      evidenceRefs: lifeModelEvidence,
      limitations: [
        "当前只是有限兼容视图，不是完整 Frontend V2。",
        "待决定和未应用建议不会被提升为长期事实。",
      ],
      fixtureId: "fixture.lifemodel.current_compatibility.v3",
    },
    {
      id: "settings-provider-privacy-unknown",
      navKey: "settings",
      shortLabel: "设置：模型与传输边界待确认",
      envelope: {
        status: "ready",
        lastUpdatedAt: "fixture-only",
        source: "backend-readmodel",
      },
      surface: { title: "设置", eyebrow: "模型与隐私" },
      primaryStatus: {
        label: "传输边界待确认",
        tone: "is-warning",
        sourceRef: "SettingsViewModel.providerPrivacyBoundary",
      },
      privacyBoundary: possibleBoundary(
        [settingsEvidence[1], settingsEvidence[2]],
        "Provider validation and runtime route evidence are unavailable in this fixture.",
      ),
      inspectorSummary: {
        happened: "本地模型已配置，但云端供应商和自动路由仍未确认。",
        risk: "旅行资料可能被发送到外部模型；当前没有运行时证明。",
        next: "保持自动路由关闭，先确认供应商、用途和传输边界。",
      },
      goal: {
        label: "当前设置",
        title: "确认旅行资料由哪个模型处理",
        summary: "在供应商与传输说明明确前，只保留本地模型作为可见配置。",
        sourceRef: "SettingsViewModel.providerPrivacyBoundary + setupReadiness",
      },
      blocker: {
        label: "隐私保护",
        title: "自动云端处理保持关闭",
        body: "选择供应商并取得路由依据之前，不把旅行资料交给外部模型。",
        tone: "is-warning",
        sourceRef: "ProviderPrivacyBoundarySummary.blockedReason",
      },
      metrics: [
        { value: "1", label: "已配置本地模型", sourceRef: "layout-only: configured local model count fixture" },
        { value: "未选择", label: "云端供应商", sourceRef: "ProviderPrivacyBoundarySummary.providerLabel" },
        { value: "关闭", label: "自动路由", sourceRef: "SettingsViewModel.setupReadiness" },
      ],
      actionPrompt: "检查隐私边界",
      actions: {
        primary: [
          {
            id: "settings:inspect-privacy",
            label: "查看传输说明",
            kind: "inspect",
            enabled: true,
            targetRef: "evidence:fixture:provider:cloud-transmission-possible",
            sourceRef: "ViewModelEnvelope.actions.primary",
            fixtureBehavior: {
              type: "open_inspector",
              evidenceId: "fixture:provider:cloud-transmission-possible",
            },
          },
          {
            id: "settings:configure-provider",
            label: "选择云端供应商",
            kind: "configure",
            enabled: false,
            disabledReason: "仅样式：静态样例不保存模型配置。",
            targetRef: "settings:provider",
            sourceRef: "ViewModelEnvelope.actions.primary",
          },
        ],
        review: [],
        debugOnly: [rawJsonAction("settings:raw-json", "fixture:settings:local-model-configured")],
      },
      sections: [
        {
          label: "当前配置",
          title: "已确认与未确认的边界",
          rows: [
            {
              title: "本地模型",
              body: "已有一项本地配置，但本页不验证实际可用性。",
              meta: "已配置",
              sourceRef: "ProviderPrivacyBoundarySummary.providerLabel",
            },
            {
              title: "云端模型",
              body: "尚未选择供应商，也没有当前运行时路由依据。",
              meta: "未选择",
              sourceRef: "ProviderPrivacyBoundarySummary.externalTransmission",
            },
          ],
        },
      ],
      evidenceRefs: settingsEvidence,
      limitations: [
        "可能外传不是已发送，也不是本地安全证明。",
        "静态设置不会保存配置或发起模型调用。",
      ],
      fixtureId: "fixture.settings.provider_privacy_unknown.v3",
    },
  ];
})();
