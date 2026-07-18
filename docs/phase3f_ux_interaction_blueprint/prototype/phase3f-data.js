(function () {
  "use strict";

  const action = config => ({
    id: config.id,
    kind: config.kind,
    label: config.label,
    enabled: Boolean(config.enabled),
    disabledReason: config.enabled ? "" : config.disabledReason,
    targetRef: config.targetRef,
    lane: config.lane || "primary",
    outcome: config.outcome || "feedback",
    sourceRef: config.sourceRef,
  });

  const evidence = (id, label, source, sensitivity, summary) => ({
    id,
    label,
    source,
    sensitivity,
    summary,
  });

  const cloneFixture = value => JSON.parse(JSON.stringify(value));

  const privacyUnknown = {
    title: "传输边界待确认",
    meta: "未证明全部在本地处理",
    tone: "warning",
    externalTransmission: "unknown",
    sourceRef: "ProviderPrivacyBoundarySummary",
  };

  const privacyLocal = {
    title: "本次动作保持本地",
    meta: "本地工具 · 未向外部发送",
    tone: "success",
    externalTransmission: "not_sent",
    sourceRef: "TARGET_CONTRACT over ProviderPrivacyBoundarySummary + task receipt",
  };

  const privacyExternal = {
    title: "本次检索会访问外部网络",
    meta: "只发送检索词，不发送附件正文",
    tone: "warning",
    externalTransmission: "sent",
    sourceRef: "TARGET_CONTRACT over task provider/network evidence",
  };

  const commonEvidence = [
    evidence(
      "ev:privacy-boundary:current",
      "模型与传输边界",
      "ProviderPrivacyBoundarySummary",
      "敏感",
      "当前没有足够依据证明相关内容只在本地处理。"
    ),
  ];

  const todayInspector = {
    summary: {
      happened: "今日计划可查看，其中一项长期偏好建议等待你的决定。",
      risk: "建议尚未改变 LifeModel；模型与传输边界仍未确认。",
      next: "先查看建议差异，再选择批准、拒绝、修改或稍后处理。",
    },
    privacy: privacyUnknown,
    evidence: [
      evidence(
        "ev:today-plan:2026-07-18",
        "今日计划快照",
        "Today projection candidate",
        "本地私密",
        "包含上午专注时段、午后复诊和当前任务关系。"
      ),
      evidence(
        "ev:review:deep-work-preference",
        "深度工作偏好建议",
        "ReviewItem + proposed decision projection",
        "敏感",
        "三次晨间计划形成的长期偏好候选，尚未批准。"
      ),
      ...commonEvidence,
    ],
    limitations: [
      "今日内容使用静态视觉 fixture，不代表实时后端状态。",
      "建议未批准、未应用，也未写入 LifeModel。",
      "传输边界未知时不显示绿色本地确定态。",
    ],
    technical: {
      routeType: "blueprint.today.ready_pending_review",
      viewModel: "TodayViewModel target over LifeStateProjection",
      updatedAt: "2026-07-18T08:42:00+08:00",
    },
  };

  const screens = {
    "today-ready": {
      key: "today-ready",
      selectorLabel: "今日 · 可用，有一项待决定建议",
      routeKey: "today",
      layout: "today",
      eyebrow: "每日工作面",
      title: "今日",
      subtitle: "7 月 18 日，星期六",
      status: {
        label: "今日计划可用",
        tone: "success",
        sourceRef: "LifeStateProjection + Today projection candidate",
      },
      privacy: privacyUnknown,
      focus: {
        kicker: "当前重点",
        title: "上午完成客户方案初稿",
        summary: "保留 09:00—11:00 的连续时间，先完成结构和关键数据核对。",
        sourceRef: "LAYOUT_FIXTURE: Today focus density",
      },
      facts: [
        {
          label: "专注时段",
          value: "09:00—11:00",
          sourceRef: "LAYOUT_FIXTURE: schedule",
        },
        {
          label: "下次提醒",
          value: "14:20",
          sourceRef: "LAYOUT_FIXTURE: reminder",
        },
        {
          label: "待决定",
          value: "1 项",
          sourceRef: "ReviewCenterViewModel.summary.pending",
        },
      ],
      attention: {
        kicker: "需要你决定",
        title: "是否把上午深度工作设为长期偏好",
        body: "建议只在审核中心等待，不会因为出现在今日页就改变长期状态。",
        tone: "warning",
        sourceRef: "ReviewItem.status + materializationStatus",
      },
      actions: [
        action({
          id: "today:view-pending-review",
          kind: "navigate",
          label: "查看待决定建议",
          enabled: true,
          targetRef: "screen:review-pending",
          outcome: "review-pending",
          sourceRef: "ViewModelEnvelope.actions.primary",
        }),
        action({
          id: "today:view-evidence",
          kind: "inspect",
          label: "查看今日依据",
          enabled: true,
          targetRef: "evidence:today",
          outcome: "inspector",
          sourceRef: "ViewModelEnvelope.actions.primary",
        }),
      ],
      schedule: [
        {
          time: "09:00",
          title: "完成方案结构",
          body: "先写清问题、方案和验证数据，暂不发送给外部收件人。",
          state: "active",
          meta: "进行中",
          sourceRef: "LAYOUT_FIXTURE: Today schedule row",
        },
        {
          time: "14:20",
          title: "康复复诊",
          body: "预留 30 分钟路程，提醒已设为本地日程参考。",
          state: "upcoming",
          meta: "今天",
          sourceRef: "LAYOUT_FIXTURE: Today schedule row",
        },
      ],
      boundaries: [
        {
          title: "不会自动发送方案",
          body: "草稿完成后仍需你确认收件人与内容。",
          icon: "shield",
          sourceRef: "DurableWritePolicy target behavior",
        },
        {
          title: "不会自动改变长期偏好",
          body: "审核前，建议与当前 LifeModel 分离。",
          icon: "history",
          sourceRef: "ReviewItem.materializationStatus",
        },
      ],
      inspector: todayInspector,
    },

    "today-stale": {
      key: "today-stale",
      selectorLabel: "今日 · 陈旧 / 未知，保护性关闭",
      routeKey: "today",
      layout: "today-stale",
      eyebrow: "每日工作面",
      title: "今日",
      subtitle: "上次可靠更新：昨天 21:10",
      status: {
        label: "计划信息已陈旧",
        tone: "warning",
        sourceRef: "ViewModelEnvelope.freshness",
      },
      privacy: privacyUnknown,
      focus: {
        kicker: "当前状态",
        title: "暂不生成新的今日建议",
        summary: "可靠计划没有刷新，OpenLife 只保留上次内容供你参考，不执行新的外部动作。",
        sourceRef: "Fail-closed presentation over stale projection",
      },
      facts: [
        {
          label: "可靠快照",
          value: "昨天",
          sourceRef: "ViewModelEnvelope.lastUpdatedAt",
        },
        {
          label: "可执行动作",
          value: "0",
          sourceRef: "ViewModelEnvelope.actions.primary[].enabled",
        },
      ],
      attention: {
        kicker: "安全模式",
        title: "陈旧信息只读展示",
        body: "刷新成功前，不根据旧计划创建任务、发送内容或改变长期状态。",
        tone: "safe",
        sourceRef: "LifeStateProjection.safeMode + freshness",
      },
      actions: [
        action({
          id: "today:request-refresh",
          kind: "refresh",
          label: "尝试刷新",
          enabled: true,
          targetRef: "today-projection",
          outcome: "refresh-feedback",
          sourceRef: "ViewModelEnvelope.actions.primary",
        }),
        action({
          id: "today:inspect-stale",
          kind: "inspect",
          label: "查看缺失依据",
          enabled: true,
          targetRef: "evidence:today-stale",
          outcome: "inspector",
          sourceRef: "ViewModelEnvelope.actions.primary",
        }),
      ],
      schedule: [
        {
          time: "昨日",
          title: "客户方案初稿",
          body: "这是上次可靠快照中的未完成项，只作参考。",
          state: "stale",
          meta: "已陈旧",
          sourceRef: "LAYOUT_FIXTURE over stale snapshot",
        },
      ],
      boundaries: [
        {
          title: "不从旧数据推断今天",
          body: "缺少新投影时不补全、不猜测。",
          icon: "shield",
          sourceRef: "Fail-closed UI rule",
        },
      ],
      inspector: {
        summary: {
          happened: "今日投影已经超过可靠时限。",
          risk: "旧计划可能不再符合今天的日程和权限边界。",
          next: "可以尝试刷新；刷新失败时继续只读。",
        },
        privacy: privacyUnknown,
        evidence: [
          evidence(
            "ev:today-stale:last-known",
            "上次可靠今日快照",
            "ViewModelEnvelope.lastUpdatedAt",
            "本地私密",
            "最后更新时间为昨天 21:10。"
          ),
          ...commonEvidence,
        ],
        limitations: ["本视觉稿不会连接后端刷新命令。", "旧快照不作为新任务或长期状态依据。"],
        technical: {
          routeType: "blueprint.today.stale_unknown",
          freshness: "stale",
          safeMode: true,
        },
      },
    },

    workspace: {
      key: "workspace",
      selectorLabel: "工作区 · 精确一次性权限，可决定并继续",
      routeKey: "workspace",
      layout: "workspace",
      eyebrow: "协作执行",
      title: "工作区",
      subtitle: "整理杭州周末行程",
      status: {
        label: "等待访问决定",
        tone: "warning",
        sourceRef: "WorkspaceViewModel.lifecycle + ReviewItem.status",
      },
      privacy: privacyLocal,
      task: {
        id: "task:hangzhou-weekend",
        kicker: "当前任务",
        title: "整理杭州周末行程",
        summary: "把已有票据和预订整理成一份可复查的两日行程草稿。",
        sourceRef: "LAYOUT_FIXTURE over limited WorkspaceViewModel",
      },
      timeline: [
        {
          id: "event:understood",
          state: "done",
          label: "已完成",
          title: "已建立行程结构",
          body: "城市、日期和交通段落已经准备好。",
          meta: "本地",
          sourceRef: "WorkspaceViewModel.timeline[] target",
        },
        {
          id: "event:file-permission",
          state: "waiting",
          label: "需要你决定",
          title: "读取“旅行/杭州周末”",
          body: "核对车票、酒店和活动日期后，再继续生成行程草稿。",
          meta: "尚未执行",
          sourceRef: "ReviewItem.status + WorkspaceViewModel.timeline[] target",
        },
        {
          id: "event:draft-itinerary",
          state: "queued",
          label: "下一步",
          title: "生成可复查的行程草稿",
          body: "访问决定明确后才会继续。",
          meta: "等待",
          sourceRef: "TasksViewModel lifecycle target",
        },
      ],
      permission: {
        title: "读取 4 份行程文件",
        purpose: "仅用于核对车次、酒店与活动日期。",
        tool: "本地文件读取",
        capability: "读取指定文件",
        target: "已选择的 4 份行程 PDF",
        dataScope: "4 份 PDF；不读取父目录或其他文件",
        transmission: "本地读取；本次动作未向外部发送",
        duration: "只对当前阻塞动作生效一次",
        revocation: "执行时即消费；不会留下持续授权",
        scopeKind: "action_bound",
        policy: "allow_once",
        inputDigest: "sha256:6b72…b4e1",
        blockedActionId: "action:file-read:trip-documents",
        sourceRef: "TARGET_CONTRACT over action-bound permission backend facts",
      },
      resources: [
        { id: "res:train", name: "往返车票.pdf", state: "ready", meta: "已导入" },
        { id: "res:hotel", name: "酒店确认单.pdf", state: "ready", meta: "已导入" },
        { id: "res:event-a", name: "展览预订.pdf", state: "ready", meta: "已导入" },
        { id: "res:event-b", name: "餐厅确认单.pdf", state: "ready", meta: "已导入" },
      ],
      actions: [
        action({
          id: "workspace:reject-permission",
          kind: "reject",
          label: "拒绝",
          enabled: true,
          targetRef: "review:permission:hangzhou-files",
          lane: "review",
          outcome: "permission-reject",
          sourceRef: "ReviewItem.allowedActions",
        }),
        action({
          id: "workspace:view-scope",
          kind: "inspect",
          label: "查看访问范围",
          enabled: true,
          targetRef: "permission:hangzhou-files",
          lane: "review",
          outcome: "inspector",
          sourceRef: "ViewModelEnvelope.actions.review + proposed permission context",
        }),
        action({
          id: "workspace:allow-once",
          kind: "approve",
          label: "仅允许本次并继续",
          enabled: true,
          targetRef: "permission:hangzhou-files",
          lane: "review",
          outcome: "permission-confirm-and-resume",
          sourceRef: "TARGET_CONTRACT over ReviewAction + TaskControl",
        }),
        action({
          id: "workspace:view-task-evidence",
          kind: "inspect",
          label: "查看任务依据",
          enabled: true,
          targetRef: "evidence:workspace-task",
          outcome: "inspector",
          sourceRef: "ViewModelEnvelope.actions.primary",
        }),
      ],
      inspector: {
        summary: {
          happened: "任务暂停在读取四份行程文件之前。",
          risk: "批准会消费一次精确动作授权；目标或输入变化时不会继续。",
          next: "核对范围后拒绝，或只允许当前动作并继续。",
        },
        privacy: privacyLocal,
        permission: true,
        evidence: [
          evidence(
            "ev:task:hangzhou-weekend",
            "当前任务",
            "WorkspaceViewModel target",
            "本地私密",
            "整理两日行程草稿，当前停在文件访问请求。"
          ),
          evidence(
            "ev:permission:hangzhou-files",
            "文件访问范围",
            "TARGET_CONTRACT over exact action-bound permission",
            "敏感",
            "目标为四份已选择 PDF；授权只匹配当前阻塞动作和输入摘要。"
          ),
          evidence(
            "ev:privacy:local-file-action",
            "本次动作传输边界",
            "ProviderPrivacyBoundarySummary + task receipt target",
            "敏感",
            "该动作是本地文件读取，fixture 表示 not_sent。"
          ),
        ],
        limitations: [
          "后端已支持精确 action-bound allow-once，但当前 ReviewItem 仍未投影这份可读上下文。",
          "本场景属于 TARGET_CONTRACT，不代表生产 UI 已可安全启用。",
          "静态原型不会读取任何文件。",
        ],
        technical: {
          routeType: "phase3f.workspace.exact_permission_target",
          taskId: "task:hangzhou-weekend",
          permissionRef: "review:permission:hangzhou-files",
          scopeKind: "action_bound",
          policy: "allow_once",
        },
      },
    },

    tasks: {
      key: "tasks",
      selectorLabel: "任务 · 活动、等待与历史",
      routeKey: "tasks",
      layout: "tasks",
      eyebrow: "连续工作",
      title: "任务",
      subtitle: "正在进行与最近完成",
      status: {
        label: "2 项需要关注",
        tone: "info",
        sourceRef: "TasksViewModel.summary target",
      },
      privacy: privacyUnknown,
      filters: ["全部", "进行中", "需要我", "已完成"],
      taskItems: [
        {
          id: "task:hangzhou-weekend",
          title: "整理杭州周末行程",
          state: "waiting",
          stateLabel: "等待访问决定",
          summary: "文件读取尚未执行；确认范围后继续生成草稿。",
          updated: "刚刚",
          next: "查看权限",
          sourceRef: "TasksViewModel.items[] target",
        },
        {
          id: "task:client-proposal",
          title: "完成客户方案初稿",
          state: "running",
          stateLabel: "整理中",
          summary: "结构已完成，正在核对最后两组数据。",
          updated: "8 分钟前",
          next: "查看进度",
          sourceRef: "TasksViewModel.items[] target",
        },
        {
          id: "task:rehab-followup",
          title: "整理康复复诊问题",
          state: "done",
          stateLabel: "已完成",
          summary: "已生成四个复诊问题，没有发送或写入外部日历。",
          updated: "昨天",
          next: "查看结果",
          sourceRef: "TasksViewModel.items[] target",
        },
      ],
      selectedTask: {
        title: "整理杭州周末行程",
        status: "等待访问决定",
        objective: "把车票、酒店和活动信息整理为可复查的两日草稿。",
        nextAction: "决定是否允许读取指定文件夹中的四份 PDF。",
        events: ["已理解两日行程目标", "已建立交通、住宿和活动结构", "等待文件访问决定"],
        sourceRef: "LAYOUT_FIXTURE over TasksViewModel detail target",
      },
      actions: [
        action({
          id: "tasks:open-workspace",
          kind: "navigate",
          label: "回到工作区",
          enabled: true,
          targetRef: "screen:workspace",
          outcome: "workspace",
          sourceRef: "TasksViewModel.items[].actions target",
        }),
        action({
          id: "tasks:inspect",
          kind: "inspect",
          label: "查看任务依据",
          enabled: true,
          targetRef: "evidence:task:hangzhou-weekend",
          outcome: "inspector",
          sourceRef: "TasksViewModel.items[].actions target",
        }),
      ],
      inspector: {
        summary: {
          happened: "一个任务等待访问决定，另一个任务仍在整理数据。",
          risk: "等待权限的任务不会自动继续。",
          next: "回到工作区处理访问请求，或查看其他任务进度。",
        },
        privacy: privacyUnknown,
        evidence: [
          evidence(
            "ev:tasks:active-summary",
            "活动任务摘要",
            "TasksViewModel target",
            "本地私密",
            "三项连续任务的生命周期和下一控制。"
          ),
          ...commonEvidence,
        ],
        limitations: [
          "本场景只验证 Tasks 信息结构，不证明生产路由已经实现。",
          "任务标题和时间为布局 fixture。",
        ],
        technical: {
          routeType: "blueprint.tasks.active_history",
          selectedTaskId: "task:hangzhou-weekend",
        },
      },
    },

    "review-pending": {
      key: "review-pending",
      selectorLabel: "审核中心 · 偏好建议等待决定",
      routeKey: "review",
      layout: "review",
      eyebrow: "建议与权限",
      title: "审核中心",
      subtitle: "1 项建议等待决定",
      status: {
        label: "等待你的决定",
        tone: "warning",
        sourceRef: "ReviewItem.status",
      },
      privacy: privacyUnknown,
      queue: [
        {
          id: "review:deep-work",
          type: "长期偏好",
          title: "工作日上午优先安排深度工作",
          meta: "低风险 · 7 天后过期",
          state: "pending",
          sourceRef: "ReviewItem + proposed decision projection",
        },
        {
          id: "review:permission:hangzhou-files",
          type: "访问请求",
          title: "读取杭州周末行程文件",
          meta: "精确一次性范围 · 目标投影",
          state: "pending",
          sourceRef: "TARGET_CONTRACT over action-bound permission",
        },
      ],
      proposal: {
        kicker: "建议变更",
        title: "工作日上午优先安排深度工作",
        summary: "以后生成工作日计划时，优先保留 09:00—11:00 的连续专注时间。",
        before: "工作日上午没有固定偏好。",
        after: "工作日 09:00—11:00 优先安排深度工作。",
        reason: "最近三次晨间计划中，你都主动把需要专注的工作移动到上午。",
        source: "晨间计划记录（3 次）",
        risk: "低风险；影响未来建议，不直接修改现有日历。",
        impact: "LifeModel 的时间偏好，以及后续每日计划排序。",
        expires: "7 天后过期，过期后需要重新提出。",
        sourceRef: "PROPOSED_REVIEW_PROJECTION",
      },
      actions: [
        action({
          id: "review:reject",
          kind: "reject",
          label: "拒绝",
          enabled: true,
          targetRef: "review:deep-work",
          lane: "review",
          outcome: "review-reject",
          sourceRef: "ReviewItem.allowedActions",
        }),
        action({
          id: "review:later",
          kind: "defer",
          label: "稍后处理",
          enabled: true,
          targetRef: "review:deep-work",
          lane: "review",
          outcome: "review-later",
          sourceRef: "ReviewItem.allowedActions",
        }),
        action({
          id: "review:edit",
          kind: "edit",
          label: "修改",
          enabled: true,
          targetRef: "review:deep-work",
          lane: "review",
          outcome: "review-edit",
          sourceRef: "ReviewItem.allowedActions",
        }),
        action({
          id: "review:approve",
          kind: "approve",
          label: "批准变更",
          enabled: true,
          targetRef: "review:deep-work",
          lane: "review",
          outcome: "review-confirm-approve",
          sourceRef: "ReviewItem.allowedActions",
        }),
        action({
          id: "review:inspect",
          kind: "inspect",
          label: "查看依据",
          enabled: true,
          targetRef: "evidence:review:deep-work",
          outcome: "inspector",
          sourceRef: "ViewModelEnvelope.actions.primary",
        }),
      ],
      inspector: {
        summary: {
          happened: "OpenLife 建议把上午深度工作作为长期偏好。",
          risk: "批准会影响未来计划建议，但不会直接改动现有日历。",
          next: "比较当前与建议，再选择拒绝、稍后、修改或批准。",
        },
        privacy: privacyUnknown,
        evidence: [
          evidence(
            "ev:review:deep-work:morning-plans",
            "三次晨间计划",
            "AgentProposal.source_detail target",
            "敏感",
            "每次都把高专注工作移动到上午。"
          ),
          evidence(
            "ev:review:deep-work:current-model",
            "当前时间偏好",
            "LifeModelViewModel target",
            "敏感",
            "当前没有固定的工作日上午偏好。"
          ),
          ...commonEvidence,
        ],
        limitations: [
          "当前 ReviewItem 尚未投影 before/after、原因和影响摘要。",
          "这些内容属于 PROPOSED_REVIEW_PROJECTION。",
          "批准只记录决定，不代表应用完成。",
        ],
        technical: {
          routeType: "blueprint.review.pending_decision",
          reviewItemId: "review:deep-work",
          projectionStatus: "PROPOSED_REVIEW_PROJECTION",
        },
      },
    },

    "review-approved": {
      key: "review-approved",
      selectorLabel: "审核中心 · 已批准，尚未应用",
      routeKey: "review",
      layout: "review-approved",
      eyebrow: "建议与权限",
      title: "审核中心",
      subtitle: "决定已记录",
      status: {
        label: "已批准，尚未应用",
        tone: "warning",
        sourceRef: "ReviewItem.status + materializationStatus",
      },
      privacy: privacyUnknown,
      result: {
        kicker: "当前结果",
        title: "深度工作偏好已批准",
        summary: "决定已经记录，但当前 LifeModel 仍保持不变。",
        decision: "已批准",
        application: "尚未应用",
        currentTruth: "工作日上午没有固定偏好",
        sourceRef: "ReviewItem.status + materializationStatus",
      },
      actions: [
        action({
          id: "review-approved:inspect",
          kind: "inspect",
          label: "查看应用依据",
          enabled: true,
          targetRef: "evidence:review:deep-work:application",
          outcome: "inspector",
          sourceRef: "ViewModelEnvelope.actions.primary",
        }),
        action({
          id: "review-approved:apply",
          kind: "apply",
          label: "应用变更",
          enabled: false,
          disabledReason: "当前没有可用的应用命令，不能把批准显示成完成",
          targetRef: "review:deep-work",
          lane: "review",
          sourceRef: "ReviewItem.allowedActions + command gap",
        }),
        action({
          id: "review-approved:return",
          kind: "navigate",
          label: "返回今日",
          enabled: true,
          targetRef: "screen:today-ready",
          outcome: "today-ready",
          sourceRef: "VISUAL_STATE navigation",
        }),
      ],
      inspector: {
        summary: {
          happened: "用户批准了深度工作偏好建议。",
          risk: "尚无应用结果；把它显示为完成会误导用户。",
          next: "等待可确认的应用命令与刷新后的读模型。",
        },
        privacy: privacyUnknown,
        evidence: [
          evidence(
            "ev:review:deep-work:decision",
            "批准决定记录",
            "ReviewItem.status",
            "敏感",
            "决定为 approved，应用状态仍为 unknown/not_applied。"
          ),
          evidence(
            "ev:lifemodel:current-time-preference",
            "当前 LifeModel 时间偏好",
            "LifeModelViewModel",
            "敏感",
            "刷新前仍没有工作日上午固定偏好。"
          ),
          ...commonEvidence,
        ],
        limitations: ["静态原型不会发出应用命令。", "只有刷新后的 applied 状态可以显示已完成。"],
        technical: {
          routeType: "blueprint.review.approved_not_applied",
          decisionStatus: "approved",
          materializationStatus: "unknown",
        },
      },
    },

    lifemodel: {
      key: "lifemodel",
      selectorLabel: "LifeModel · 当前兼容视图受限",
      routeKey: "lifemodel",
      layout: "lifemodel",
      eyebrow: "长期理解",
      title: "LifeModel",
      subtitle: "OpenLife 当前怎样理解你",
      status: {
        label: "当前视图受限",
        tone: "warning",
        sourceRef: "LifeModelViewModel.truthMode + contractLimitations",
      },
      privacy: privacyUnknown,
      summary: {
        kicker: "当前理解",
        title: "会议前预留 15 分钟准备时间",
        body: "这条偏好有三个来源；上午深度工作建议仍在审核流程外，不会混入当前理解。",
        sourceRef: "LifeModelViewModel.currentViewSummary",
      },
      tabs: ["概览", "目标", "偏好", "关系", "记忆与来源"],
      dimensions: [
        {
          label: "节奏偏好",
          title: "会议前保留准备缓冲",
          body: "通常在会议前安排 15 分钟整理材料和确认目标。",
          provenance: "3 条来源",
          confidence: "当前",
          sourceRef: "LAYOUT_FIXTURE over LifeModelViewModel current view",
        },
        {
          label: "工作方式",
          title: "先完成结构，再补充细节",
          body: "在需要交付的写作任务中，倾向先确认结构和验证标准。",
          provenance: "2 条来源",
          confidence: "当前",
          sourceRef: "LAYOUT_FIXTURE over LifeModelViewModel current view",
        },
      ],
      pending: {
        title: "上午深度工作偏好",
        body: "等待你的决定；当前视图不会提前采用。",
        sourceRef: "LifeModelViewModel.pendingUpdateCounts + ReviewItem.status",
      },
      actions: [
        action({
          id: "lifemodel:view-source",
          kind: "inspect",
          label: "查看来源",
          enabled: true,
          targetRef: "evidence:lifemodel:meeting-buffer",
          outcome: "inspector",
          sourceRef: "ViewModelEnvelope.actions.primary",
        }),
        action({
          id: "lifemodel:view-pending",
          kind: "navigate",
          label: "查看待决定建议",
          enabled: true,
          targetRef: "screen:review-pending",
          outcome: "review-pending",
          sourceRef: "ViewModelEnvelope.actions.primary",
        }),
      ],
      inspector: {
        summary: {
          happened: "当前兼容视图显示两条已有偏好和来源。",
          risk: "待决定或尚未应用的建议不能进入当前长期理解。",
          next: "查看来源；要改变偏好时进入审核中心。",
        },
        privacy: privacyUnknown,
        evidence: [
          evidence(
            "ev:lifemodel:meeting-buffer",
            "会议缓冲偏好来源",
            "LifeModelViewModel.provenanceRefs",
            "敏感",
            "来自两次手动调整和一次明确反馈。"
          ),
          evidence(
            "ev:lifemodel:compatibility",
            "当前兼容视图边界",
            "LifeModelViewModel.contractLimitations",
            "产品状态",
            "当前只展示有来源的受限视图。"
          ),
          ...commonEvidence,
        ],
        limitations: [
          "兼容视图不是完整 canonical LifeModel 浏览器。",
          "待决定建议不会混入当前理解。",
        ],
        technical: {
          routeType: "blueprint.lifemodel.limited_compatibility",
          truthMode: "limited_compatibility",
          pendingUpdateCount: 1,
        },
      },
    },

    settings: {
      key: "settings",
      selectorLabel: "设置 · 模型与隐私边界未知",
      routeKey: "settings",
      layout: "settings",
      eyebrow: "产品设置",
      title: "设置",
      subtitle: "模型、隐私与权限",
      status: {
        label: "传输边界待确认",
        tone: "warning",
        sourceRef: "ProviderPrivacyBoundarySummary",
      },
      privacy: privacyUnknown,
      categories: [
        { id: "models", label: "模型与供应商", keywords: "模型 供应商 API Key endpoint base url" },
        { id: "privacy", label: "隐私与网络", keywords: "隐私 网络 外传 本地 policy" },
        { id: "tools", label: "工具与权限", keywords: "工具 权限 文件 allow once" },
        { id: "data", label: "数据与恢复", keywords: "导入 导出 快照 恢复" },
        { id: "memory", label: "LifeModel 与记忆", keywords: "LifeModel 记忆 审核 回滚" },
        { id: "appearance", label: "外观", keywords: "外观 主题 字体" },
        { id: "advanced", label: "高级与支持", keywords: "高级 日志 版本 诊断" },
      ],
      hero: {
        kicker: "模型与隐私",
        title: "配置模型，不替后端判断传输边界",
        body: "本地模型已配置，但这不证明所有请求都会留在本地。修改供应商、模型或地址后，边界保持未知，直到后端重新确认。",
        sourceRef: "AppConfig + ProviderPrivacyBoundarySummary",
      },
      config: {
        preferLocal: true,
        provider: "deepseek",
        model: "deepseek-chat",
        endpoint: "https://api.deepseek.com",
        credential: "",
        credentialPlaceholder: "已安全保存；留空表示不替换",
      },
      categoryContent: {
        privacy: {
          title: "隐私与网络",
          summary: "查看当前请求是否可能外传，以及哪条网络策略在生效。",
        },
        tools: {
          title: "工具与权限",
          summary: "管理按次确认和可撤销授权；一次性授权执行后不会留下持续开关。",
        },
        data: {
          title: "数据与恢复",
          summary: "导出、导入和恢复均需要预检；危险操作不会从普通设置行直接执行。",
        },
        memory: {
          title: "LifeModel 与记忆",
          summary: "长期理解和记忆写入保持审核优先，并区分候选、已批准与已应用。",
        },
        appearance: {
          title: "外观",
          summary: "Phase 3F 冻结白色工作台基线；生产主题切换不在本阶段实现。",
        },
        advanced: {
          title: "高级与支持",
          summary: "版本、诊断和原始字段默认收起；开发扩展不会进入生产主导航。",
        },
      },
      actions: [
        action({
          id: "settings:view-privacy",
          kind: "inspect",
          label: "查看传输说明",
          enabled: true,
          targetRef: "evidence:privacy-boundary",
          outcome: "inspector",
          sourceRef: "ViewModelEnvelope.actions.primary",
        }),
        action({
          id: "settings:test-provider",
          kind: "configure",
          label: "测试连接",
          enabled: true,
          targetRef: "settings:draft-provider",
          outcome: "settings-test-connection",
          sourceRef: "PRODUCT_BRIDGE: testLlmConnection",
        }),
        action({
          id: "settings:save-provider",
          kind: "configure",
          label: "保存设置",
          enabled: true,
          targetRef: "settings:draft-provider",
          outcome: "settings-save",
          sourceRef: "PRODUCT_BRIDGE: saveConfig + refresh boundary",
        }),
      ],
      inspector: {
        summary: {
          happened: "本地优先与一个云端供应商配置同时存在，当前传输边界仍为未知。",
          risk: "配置值、连接测试和实际请求路线是三种不同事实。",
          next: "检查供应商和地址；测试只验证一次，保存后等待后端刷新边界。",
        },
        privacy: privacyUnknown,
        evidence: [
          evidence(
            "ev:settings:provider-boundary",
            "供应商与传输摘要",
            "ProviderPrivacyBoundarySummary",
            "敏感",
            "当前外传状态未知；页面不会从 provider 配置推断本地或私密。"
          ),
          evidence(
            "ev:settings:local-model",
            "本地模型配置",
            "AppConfig sanitized read",
            "产品状态",
            "仅证明存在本地配置，不证明当前请求路线。"
          ),
        ],
        limitations: [
          "测试与保存均为静态状态机演示，不连接生产设置或网络。",
          "页面不会从配置数量推断本地/私密真值。",
        ],
        technical: {
          routeType: "phase3f.settings.provider_privacy_unknown",
          providerLabel: "deepseek_fixture",
          externalTransmission: "unknown",
          composition: "AppConfig + LlmConnectionTestResult + ProviderPrivacyBoundarySummary",
        },
      },
    },
  };

  const workspaceUnknown = cloneFixture(screens.workspace);
  workspaceUnknown.key = "workspace-unknown";
  workspaceUnknown.selectorLabel = "工作区 · 权限范围或传输边界未知，保护性关闭";
  workspaceUnknown.subtitle = "整理杭州周末行程 · 缺少可靠范围";
  workspaceUnknown.status = {
    label: "访问范围不完整",
    tone: "warning",
    sourceRef: "ReviewItem projection gap + ProviderPrivacyBoundarySummary",
  };
  workspaceUnknown.privacy = privacyUnknown;
  workspaceUnknown.permission = {
    ...workspaceUnknown.permission,
    target: "目标未可靠投影",
    dataScope: "文件数量与边界未知",
    transmission: "未知；不能证明只在本地处理",
    duration: "未知",
    revocation: "未知",
    sourceRef: "MISSING_PERMISSION_DECISION_CONTEXT",
  };
  workspaceUnknown.actions = workspaceUnknown.actions.map(item =>
    item.id === "workspace:allow-once"
      ? {
          ...item,
          enabled: false,
          disabledReason: "缺少可读的精确动作范围或传输边界",
          outcome: "feedback",
          sourceRef: "FAIL_CLOSED_TARGET_CONTRACT_GAP",
        }
      : item
  );
  workspaceUnknown.inspector = {
    ...workspaceUnknown.inspector,
    summary: {
      happened: "任务停在文件读取之前，但当前页面拿不到完整、可核对的动作范围。",
      risk: "允许一个不可核对的范围可能扩大访问或产生未知外传。",
      next: "查看现有依据、拒绝或稍后处理；不要授权。",
    },
    privacy: privacyUnknown,
    evidence: [
      evidence(
        "ev:permission:missing-readable-scope",
        "权限投影缺口",
        "ReviewItem current contract",
        "产品状态",
        "当前只有动作引用和 EvidenceRef 元数据，无法回答完整访问范围。"
      ),
      ...commonEvidence,
    ],
    limitations: [
      "缺少 ReviewDecisionContext / PermissionDecisionContext。",
      "未知状态保持禁用，不从 fixture 或工具名推断范围。",
      "静态原型不会读取任何文件。",
    ],
    technical: {
      routeType: "phase3f.workspace.permission_unknown",
      requiredProjection: "PermissionDecisionContext",
      actionEnabled: false,
    },
  };
  screens[workspaceUnknown.key] = workspaceUnknown;

  const workspaceRunning = cloneFixture(screens.workspace);
  workspaceRunning.key = "workspace-running";
  workspaceRunning.selectorLabel = "工作区 · 决定已刷新，精确动作正在继续";
  workspaceRunning.subtitle = "整理杭州周末行程 · 已恢复";
  workspaceRunning.status = {
    label: "正在读取已选择文件",
    tone: "info",
    sourceRef: "STATIC_TRANSITION over refreshed TasksViewModel target",
  };
  workspaceRunning.timeline = [
    {
      id: "event:permission-consumed",
      state: "done",
      label: "决定已记录",
      title: "一次性授权已匹配当前动作",
      body: "静态演示先刷新审核与任务状态，再请求恢复；授权不会用于其他输入。",
      meta: "已消费",
      sourceRef: "TARGET_CONTRACT over action-bound permission receipt",
    },
    {
      id: "event:file-read-running",
      state: "running",
      label: "正在处理",
      title: "核对 4 份行程文件",
      body: "只读取已选择的车票、酒店和活动 PDF。",
      meta: "本地",
      sourceRef: "TARGET_CONTRACT over task action/observation",
    },
    {
      id: "event:draft-itinerary",
      state: "queued",
      label: "下一步",
      title: "生成可复查的行程草稿",
      body: "文件读取完成后继续；不会自动发送或写入外部日历。",
      meta: "等待",
      sourceRef: "TasksViewModel nextRecommendedControl target",
    },
  ];
  workspaceRunning.resources = [
    { id: "res:train", name: "往返车票.pdf", state: "reading", meta: "读取中" },
    { id: "res:hotel", name: "酒店确认单.pdf", state: "ready", meta: "已导入" },
    { id: "res:event-a", name: "活动预订 A.pdf", state: "ready", meta: "已导入" },
    { id: "res:event-b", name: "活动预订 B.pdf", state: "ready", meta: "已导入" },
  ];
  workspaceRunning.actions = [
    action({
      id: "workspace:view-task-evidence",
      kind: "inspect",
      label: "查看任务依据",
      enabled: true,
      targetRef: "evidence:workspace-task",
      outcome: "inspector",
      sourceRef: "ViewModelEnvelope.actions.primary",
    }),
    action({
      id: "workspace:cancel-running",
      kind: "cancel",
      label: "取消任务",
      enabled: true,
      targetRef: "task:hangzhou-weekend",
      outcome: "cancel-task-feedback",
      sourceRef: "TasksViewModel.allowedControls target",
    }),
  ];
  workspaceRunning.inspector = {
    ...workspaceRunning.inspector,
    summary: {
      happened: "静态流程已模拟批准、刷新、恢复和精确权限消费，任务现在处于运行状态。",
      risk: "真实产品仍需以后端刷新结果为准；前端不能只凭批准回调进入运行态。",
      next: "查看当前文件动作依据，或取消任务。",
    },
    limitations: [
      "这是确定性的静态状态机演示，没有调用 accept/resume 命令。",
      "生产 UI 只有在刷新后的 task lifecycle 为 running 时才能显示此状态。",
    ],
    technical: {
      routeType: "phase3f.workspace.permission_resumed_fixture",
      transition: "approve -> refresh review/task -> resume -> refresh task",
      actionBoundGrant: "consumed_once",
    },
  };
  screens[workspaceRunning.key] = workspaceRunning;

  const workspaceResearch = cloneFixture(workspaceRunning);
  workspaceResearch.key = "workspace-resources-web";
  workspaceResearch.selectorLabel = "工作区 · 附件与受治理 Web 依据";
  workspaceResearch.subtitle = "准备杭州周末行程 · 资料与公开信息";
  workspaceResearch.privacy = privacyExternal;
  workspaceResearch.status = {
    label: "正在核对公开信息",
    tone: "warning",
    sourceRef: "TARGET_CONTRACT over governed web action",
  };
  workspaceResearch.timeline = [
    {
      id: "event:resources-selected",
      state: "done",
      label: "本地资料",
      title: "已选择 2 份行程凭证",
      body: "附件正文保持本地；只把选中的相关片段用于当前回答。",
      meta: "2 个引用",
      sourceRef: "ResourceImportReceipt + ResourceCitation target",
      sources: ["往返车票.pdf", "酒店确认单.pdf"],
    },
    {
      id: "event:web-search",
      state: "running",
      label: "外部检索",
      title: "核对周六展览开放时间",
      body: "只发送检索词；搜索结果按不可信外部数据处理。",
      meta: "受治理",
      sourceRef: "WebSearchObservation + task action target",
      sources: ["museum.example", "city.example"],
    },
    {
      id: "event:result",
      state: "queued",
      label: "下一步",
      title: "生成带来源的两日草稿",
      body: "缺少或伪造引用时会保护性失败，不输出带虚假来源的正常答案。",
      meta: "等待引用验证",
      sourceRef: "ResourceCitationSet + WebCitationSet validation",
    },
  ];
  workspaceResearch.resources = [
    { id: "res:train", name: "往返车票.pdf", state: "ready", meta: "已选用" },
    { id: "res:hotel", name: "酒店确认单.pdf", state: "ready", meta: "已选用" },
  ];
  workspaceResearch.inspector = {
    summary: {
      happened: "任务结合两份本地附件和一次受治理 Web 检索来准备行程草稿。",
      risk: "检索词会发送到外部；附件正文不应随检索请求外传。",
      next: "查看本地与外部来源的分界，等待引用验证后再使用结果。",
    },
    privacy: privacyExternal,
    evidence: [
      evidence(
        "ev:resource:trip-receipt",
        "附件导入与选用回执",
        "ResourceImportReceipt + ResourceCitationSet target",
        "本地私密",
        "两份本地附件已导入并为当前任务选用。"
      ),
      evidence(
        "ev:web:opening-hours",
        "公开开放时间检索",
        "WebSearchObservation + WebCitationSet target",
        "公开",
        "检索词发送到外部；结果被标记为不可信数据并等待引用验证。"
      ),
    ],
    limitations: [
      "当前没有完整的 V2 Workspace 资源/Web 时间线读模型。",
      "域名和引用数量是布局 fixture，不是实时搜索结果。",
      "静态原型没有读取附件或访问网络。",
    ],
    technical: {
      routeType: "phase3f.workspace.resources_web_target",
      resourceCitationState: "fixture_selected",
      webCitationState: "fixture_validation_pending",
    },
  };
  screens[workspaceResearch.key] = workspaceResearch;

  const navigation = [
    {
      key: "today",
      label: "今日",
      description: "每天先看这里",
      icon: "calendar",
      screen: "today-ready",
      placement: "primary",
      mobile: true,
    },
    {
      key: "workspace",
      label: "工作区",
      description: "当前协作执行",
      icon: "workspace",
      screen: "workspace",
      placement: "primary",
      mobile: true,
    },
    {
      key: "tasks",
      label: "任务",
      description: "连续工作与恢复",
      icon: "tasks",
      screen: "tasks",
      placement: "primary",
      mobile: false,
    },
    {
      key: "review",
      label: "审核中心",
      description: "建议与权限决定",
      icon: "review",
      screen: "review-pending",
      placement: "primary",
      mobile: true,
      badge: "1",
    },
    {
      key: "lifemodel",
      label: "LifeModel",
      description: "长期理解与来源",
      icon: "lifemodel",
      screen: "lifemodel",
      placement: "primary",
      mobile: true,
    },
    {
      key: "settings",
      label: "设置",
      description: "模型、隐私与权限",
      icon: "settings",
      screen: "settings",
      placement: "utility",
      mobile: false,
    },
    {
      key: "support",
      label: "支持信息",
      description: "依据与技术详情",
      icon: "terminal",
      screen: null,
      placement: "utility",
      mobile: false,
      outcome: "inspector",
    },
  ];

  window.OPENLIFE_BLUEPRINT_DATA = {
    version: "phase3f-interaction-blueprint-v1",
    defaultScreen: "today-ready",
    navigation,
    screens,
  };
})();
