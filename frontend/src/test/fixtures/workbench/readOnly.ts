import type {
  EvidenceRef,
  ProviderPrivacyBoundarySummary,
  TaskViewModelItem,
  TasksViewModel,
  ViewModelEnvelope,
} from "@/tauri";
import {
  buildTodayViewModelEnvelope,
  type BuildTodayViewModelEnvelopeInput,
} from "@/viewmodels/today/todayViewModelAdapter";
import { makeDailyGoal, makeLifeStateProjection } from "@/viewmodels/today/todayViewModel.fixtures";
import type {
  ReadOnlySpineDataSource,
  TasksReadOnlySnapshot,
  TodayReadOnlySnapshot,
} from "@/ui/journeys/readOnly";

export type WorkbenchFixtureId =
  | "fixture-ready"
  | "fixture-incomplete-permission"
  | "fixture-stale"
  | "fixture-error"
  | "fixture-empty"
  | "fixture-durable-approved"
  | "fixture-durable-applying"
  | "fixture-durable-applied"
  | "fixture-durable-failed"
  | "fixture-durable-rolled-back"
  | "fixture-settings-local-known"
  | "fixture-settings-review-required"
  | "fixture-settings-refresh-unknown"
  | "fixture-settings-save-failed";

const generatedAt = "2026-07-18T08:30:00.000Z";

const localBoundaryEvidence: EvidenceRef = {
  id: "provider-route:local-ollama:qwen2.5-14b",
  label: "本次模型路由记录",
  source: "provider",
  sensitivity: "local_private",
};

const localBoundary: ProviderPrivacyBoundarySummary = {
  routeType: "local",
  externalTransmission: "not_sent",
  providerLabel: "Ollama",
  modelLabel: "qwen2.5:14b",
  privacyLabel: "仅本机处理",
  risk: "none",
  localOnlyRequired: true,
  evidenceRefs: [localBoundaryEvidence],
};

function boundaryEnvelope(
  status: "ready" | "stale" | "error",
  data: ProviderPrivacyBoundarySummary | null
): ViewModelEnvelope<ProviderPrivacyBoundarySummary> {
  return {
    data,
    status,
    lastUpdatedAt: status === "stale" ? "2026-07-15T08:30:00.000Z" : generatedAt,
    source: "backend-readmodel",
    evidenceRefs: data?.evidenceRefs ?? [],
    warnings:
      status === "stale"
        ? [
            {
              code: "fixture.provider_boundary_stale",
              message: "The fixture boundary is stale and must not render as verified local.",
              severity: "warning",
              evidenceRefs: data?.evidenceRefs ?? [],
            },
          ]
        : status === "error"
          ? [
              {
                code: "fixture.provider_boundary_error",
                message: "The fixture boundary could not be read.",
                severity: "error",
                evidenceRefs: [],
              },
            ]
          : [],
    actions: { primary: [], review: [], debugOnly: [] },
  };
}

function taskEvidence(id: string, label: string): EvidenceRef {
  return {
    id,
    label,
    source: "task",
    sensitivity: "local_private",
  };
}

function task(
  overrides: Partial<TaskViewModelItem> & Pick<TaskViewModelItem, "canonicalTaskId" | "title">
): TaskViewModelItem {
  const { canonicalTaskId, title, ...rest } = overrides;
  return {
    canonicalTaskId,
    relatedRunIds: [],
    title,
    strategy: "react",
    lifecycleStatus: "running",
    terminalDeliveryStatus: "not_terminal",
    finalDeliveryEvidencePresent: false,
    items: [],
    artifacts: [],
    pendingBlockers: [],
    pendingReviewItemRefs: [],
    allowedControls: [],
    nextRecommendedControl: "none",
    evidenceRefs: [taskEvidence(`task:${canonicalTaskId}`, "任务生命周期记录")],
    updatedAt: generatedAt,
    ...rest,
  };
}

const readyTasks: TaskViewModelItem[] = [
  task({
    canonicalTaskId: "task-interview-notes",
    title: "整理三次客户访谈，归纳下周要验证的问题",
    lifecycleStatus: "waiting_permission",
    pendingBlockers: ["需要确认是否允许读取“访谈记录”目录；尚未访问文件。"],
    pendingReviewItemRefs: [
      {
        id: "review-permission-interview-notes",
        kind: "review_item",
        label: "读取访谈记录目录的权限请求",
      },
    ],
    nextRecommendedControl: "open_review_item",
  }),
  task({
    canonicalTaskId: "task-weekly-brief",
    title: "把本周项目进展汇总成一页周报",
    lifecycleStatus: "running",
    items: [
      {
        id: "item:weekly-brief:document-read",
        runId: "run:weekly-brief",
        sequence: 3,
        kind: "tool_call",
        status: "completed",
        summaryCode: "work_tool_call:document.read",
        evidenceRefs: [taskEvidence("item:weekly-brief:document-read", "本地文档读取")],
      },
      {
        id: "item:weekly-brief:document-observation",
        runId: "run:weekly-brief",
        sequence: 4,
        kind: "observation",
        status: "completed",
        summaryCode: "work_tool_observation:document.read",
        evidenceRefs: [taskEvidence("item:weekly-brief:document-observation", "本地文档结果")],
      },
      {
        id: "item:weekly-brief:provider",
        runId: "run:weekly-brief",
        sequence: 5,
        kind: "provider_generation",
        status: "running",
        summaryCode: "work_provider_generation",
        evidenceRefs: [taskEvidence("item:weekly-brief:provider", "模型生成状态")],
      },
    ],
    latestResultPreview: {
      status: "not_terminal",
      label: "正在整理本地笔记",
      preview: "已找到会议纪要与里程碑，仍在组织重点。",
      evidenceRefs: [taskEvidence("task-result:weekly-brief:partial", "周报阶段结果")],
    },
  }),
  task({
    canonicalTaskId: "task-travel-checklist",
    title: "生成周末出行的行前清单",
    lifecycleStatus: "completed",
    terminalDeliveryStatus: "delivered",
    finalDeliveryEvidencePresent: true,
    artifacts: [
      {
        artifactId: "artifact:travel-checklist",
        version: 1,
        status: "materialized",
        mediaType: "text/markdown; charset=utf-8",
        contentDigest: "sha256:travel-checklist-v1",
        targetReferenceDigest: "sha256:travel-checklist-target",
        materializedReference: "/OpenLife/Results/travel-checklist.md",
        observedContentDigest: "sha256:travel-checklist-v1",
        proposalRef: {
          id: "proposal:travel-checklist-v1",
          kind: "review_item",
          label: "清单写入审核",
        },
        sourceItemRef: {
          id: "item:travel-checklist-delivery",
          kind: "evidence",
          label: "清单产物草稿",
        },
        evidenceRefs: [taskEvidence("artifact:travel-checklist:v1", "清单产物版本")],
        change: {
          kind: "create",
          status: "materialized",
          targetReference: "/OpenLife/Results/travel-checklist.md",
        },
        preview: {
          status: "available",
          content: "# 周末出行清单\n\n- 证件\n- 交通\n- 天气",
        },
        verification: {
          status: "verified",
          expectedContentDigest: "sha256:travel-checklist-v1",
          observedContentDigest: "sha256:travel-checklist-v1",
          verificationItemPresent: true,
        },
        undo: {
          available: true,
        },
      },
    ],
    latestResultPreview: {
      status: "delivered",
      label: "清单已交付",
      preview: "交通、证件、天气与随身物品已经整理。",
      finalDeliveryRef: {
        id: "delivery:travel-checklist:v1",
        kind: "evidence",
        label: "最终清单",
      },
      evidenceRefs: [taskEvidence("task-delivery:travel-checklist:v1", "最终交付记录")],
    },
  }),
  task({
    canonicalTaskId: "task-focus-preference",
    title: "记录我更偏好上午安排深度工作的习惯",
    lifecycleStatus: "completed_with_pending_review",
    terminalDeliveryStatus: "completed_with_pending_review",
    pendingReviewItemRefs: [
      {
        id: "review-lifemodel-focus-preference",
        kind: "review_item",
        label: "LifeModel 更新建议",
      },
    ],
    latestResultPreview: {
      status: "completed_with_pending_review",
      label: "建议已生成，等待决定",
      preview: "尚未批准，也没有写入长期状态。",
      evidenceRefs: [taskEvidence("proposal:focus-preference", "长期状态建议来源")],
    },
  }),
  task({
    canonicalTaskId: "task-reading-export",
    title: "导出本月阅读摘录",
    lifecycleStatus: "completed_needs_evidence",
    terminalDeliveryStatus: "missing_final_delivery_evidence",
    finalDeliveryEvidencePresent: false,
    latestResultPreview: {
      status: "missing_final_delivery_evidence",
      label: "缺少最终交付证明",
      preview: "任务报告完成，但没有可核对的导出文件引用。",
      evidenceRefs: [taskEvidence("task-result:reading-export", "任务终止状态")],
    },
  }),
];

function tasksEnvelope(
  status: "ready" | "stale" | "empty" | "error",
  items: TaskViewModelItem[]
): ViewModelEnvelope<TasksViewModel> {
  const sourceRef = taskEvidence("tasks-view-model:fixture", "任务列表状态来源");
  const data: TasksViewModel = {
    items,
    summary: {
      total: items.length,
      needsAttentionCount: items.filter(item => item.needsAttention).length,
      activeCount: items.filter(item =>
        ["running", "waiting_review", "waiting_permission", "blocked"].includes(
          item.lifecycleStatus
        )
      ).length,
      waitingReviewCount: items.filter(item => item.lifecycleStatus === "waiting_review").length,
      waitingPermissionCount: items.filter(item => item.lifecycleStatus === "waiting_permission")
        .length,
      blockedCount: items.filter(item => item.lifecycleStatus === "blocked").length,
      pendingReviewCount: items.filter(item => item.pendingReviewItemRefs.length > 0).length,
      completedCount: items.filter(item => item.lifecycleStatus === "completed").length,
      completedNeedsEvidenceCount: items.filter(
        item => item.lifecycleStatus === "completed_needs_evidence"
      ).length,
      failedCount: items.filter(item => item.lifecycleStatus === "failed").length,
      cancelledCount: items.filter(item => item.lifecycleStatus === "cancelled").length,
      byLifecycleStatus: Object.fromEntries(
        items.map(item => [
          item.lifecycleStatus,
          items.filter(candidate => candidate.lifecycleStatus === item.lifecycleStatus).length,
        ])
      ),
    },
    sourceRefs: [sourceRef],
    contractLimitations: [
      "The read-only journey does not dispatch TaskControl entries.",
      "A completed lifecycle requires delivered final evidence before verified UI completion.",
    ],
  };

  return {
    data,
    status,
    lastUpdatedAt: status === "stale" ? "2026-07-15T08:30:00.000Z" : generatedAt,
    source: "backend-readmodel",
    evidenceRefs: [sourceRef],
    warnings:
      status === "stale"
        ? [
            {
              code: "fixture.tasks_stale",
              message: "TasksViewModel fixture is stale.",
              severity: "warning",
              evidenceRefs: [sourceRef],
            },
          ]
        : status === "error"
          ? [
              {
                code: "fixture.tasks_error",
                message:
                  "TasksViewModel fixture carries an untrusted empty payload with error status.",
                severity: "error",
                evidenceRefs: [sourceRef],
              },
            ]
          : [],
    actions: {
      primary: [
        {
          id: "tasks.refresh",
          label: "Refresh TasksViewModel",
          kind: "refresh",
          enabled: true,
          targetRef: "TasksViewModel",
        },
      ],
      review: [],
      debugOnly: [],
    },
  };
}

function todayInput(id: WorkbenchFixtureId): BuildTodayViewModelEnvelopeInput {
  if (id === "fixture-error") {
    return {
      projection: null,
      status: "error",
      errorMessage: "LifeStateProjection fixture could not be loaded.",
    };
  }
  if (id === "fixture-empty") {
    return {
      projection: makeLifeStateProjection({
        generatedAt,
        pending: {
          pendingProposalCount: 0,
          editedProposalCount: 0,
          totalReviewRequiredCount: 0,
          highRiskReviewRequiredCount: 0,
          proposalStoreStatus: "ok",
          requiresUserAction: false,
        },
        taskState: {
          taskStoreStatus: "ok",
          latestTaskId: null,
          latestTaskStatus: null,
          runningCount: 0,
          waitingPermissionCount: 0,
          blockedCount: 0,
          failedCount: 0,
          cancelledCount: 0,
          completedCount: 0,
          activeCount: 0,
        },
      }),
      dailyGoals: [],
      providerPrivacyBoundary: localBoundary,
      status: "empty",
    };
  }
  return {
    projection: makeLifeStateProjection({
      generatedAt,
      pending: {
        pendingProposalCount: 1,
        editedProposalCount: 0,
        totalReviewRequiredCount: 1,
        highRiskReviewRequiredCount: 0,
        proposalStoreStatus: "ok",
        requiresUserAction: true,
      },
      taskState: {
        taskStoreStatus: "ok",
        latestTaskId: "task-weekly-brief",
        latestTaskStatus: "running",
        runningCount: 1,
        waitingPermissionCount: 1,
        blockedCount: 0,
        failedCount: 0,
        cancelledCount: 0,
        completedCount: 1,
        activeCount: 2,
      },
    }),
    dailyGoals: [
      makeDailyGoal({
        name: "整理下周客户访谈要验证的三个问题",
        time_block: { start: "09:30", end: "10:30" },
      }),
    ],
    providerPrivacyBoundary: localBoundary,
    status: id === "fixture-stale" ? "stale" : "ready",
    lastUpdatedAt: id === "fixture-stale" ? "2026-07-15T08:30:00.000Z" : generatedAt,
  };
}

function makeTodaySnapshot(id: WorkbenchFixtureId): TodayReadOnlySnapshot {
  const boundaryStatus =
    id === "fixture-error" ? "error" : id === "fixture-stale" ? "stale" : "ready";
  return {
    envelope: buildTodayViewModelEnvelope(todayInput(id)),
    boundaryEnvelope: boundaryEnvelope(
      boundaryStatus,
      boundaryStatus === "error" ? null : localBoundary
    ),
    diagnostics: [
      {
        id: "life_state_projection",
        status: id === "fixture-error" ? "failed" : "loaded",
        message: id === "fixture-error" ? "Static error fixture." : undefined,
      },
      { id: "daily_goals", status: id === "fixture-error" ? "failed" : "loaded" },
      { id: "provider_privacy", status: id === "fixture-error" ? "failed" : "loaded" },
    ],
  };
}

function makeTasksSnapshot(id: WorkbenchFixtureId): TasksReadOnlySnapshot {
  const status =
    id === "fixture-error"
      ? "error"
      : id === "fixture-stale"
        ? "stale"
        : id === "fixture-empty"
          ? "empty"
          : "ready";
  const boundaryStatus =
    id === "fixture-error" ? "error" : id === "fixture-stale" ? "stale" : "ready";
  return {
    envelope: tasksEnvelope(status, id === "fixture-empty" ? [] : readyTasks),
    boundaryEnvelope: boundaryEnvelope(
      boundaryStatus,
      boundaryStatus === "error" ? null : localBoundary
    ),
    diagnostics: [
      {
        id: "tasks_view_model",
        status: id === "fixture-error" ? "failed" : "loaded",
        message: id === "fixture-error" ? "Static error fixture." : undefined,
      },
      { id: "provider_privacy", status: id === "fixture-error" ? "failed" : "loaded" },
    ],
  };
}

export const workbenchFixtureLabels: Record<WorkbenchFixtureId, string> = {
  "fixture-ready": "静态样例：可用 + 需要处理",
  "fixture-incomplete-permission": "静态样例：权限范围不完整",
  "fixture-stale": "静态样例：数据陈旧",
  "fixture-error": "静态样例：读取失败",
  "fixture-empty": "静态样例：暂无内容",
  "fixture-durable-approved": "长期状态：已批准，尚未应用",
  "fixture-durable-applying": "长期状态：正在应用",
  "fixture-durable-applied": "长期状态：已应用",
  "fixture-durable-failed": "长期状态：应用失败",
  "fixture-durable-rolled-back": "长期状态：已回滚",
  "fixture-settings-local-known": "设置：本地边界已确认",
  "fixture-settings-review-required": "设置：外部测试需要审核",
  "fixture-settings-refresh-unknown": "设置：保存后边界未知",
  "fixture-settings-save-failed": "设置：保存失败",
};

export function readOnlyFixtureDataSource(id: WorkbenchFixtureId): ReadOnlySpineDataSource {
  return {
    async loadToday() {
      return makeTodaySnapshot(id);
    },
    async loadTasks() {
      return makeTasksSnapshot(id);
    },
  };
}
