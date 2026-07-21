import type {
  EvidenceRef,
  ProviderPrivacyBoundarySummary,
  ReviewAction,
  ReviewCenterViewModel,
  ReviewItem,
  TaskControl,
  TasksViewModel,
  TaskViewModelItem,
  ViewModelEnvelope,
  ViewModelStatus,
  WorkspaceViewModel,
} from "@/tauri";
import type {
  GovernedActionDataSource,
  GovernedActionSnapshot,
} from "@/ui/journeys/governedAction";
import type { DurableTruthDataSource } from "@/ui/journeys/durableTruth";
import type { ReadOnlySpineDataSource } from "@/ui/journeys/readOnly";
import type { SettingsPrivacyDataSource } from "@/ui/journeys/settingsPrivacy";
import {
  buildDurableFixtureSnapshot,
  durableReviewItem,
  durableReviewItemId,
  initialDurableFixtureStage,
  type DurableFixtureStage,
} from "./phase4d-durable-fixtures";
import { phase4dFixtureDataSource, type Phase4dFixtureId } from "./phase4d-fixtures";
import {
  createPhase4dSettingsFixture,
  providerTestReviewItem,
  providerTestReviewItemId,
  type ProviderTestFixtureStage,
} from "./phase4d-settings-fixtures";

export type Phase4dJourneyDataSource = ReadOnlySpineDataSource &
  GovernedActionDataSource &
  DurableTruthDataSource &
  SettingsPrivacyDataSource;

type FixtureStage = "pending" | "approved" | "rejected" | "deferred" | "running";

const generatedAt = "2026-07-20T09:30:00.000Z";
const taskId = "task-interview-notes";
const reviewItemId = "review-permission-interview-notes";

const permissionEvidence: EvidenceRef = {
  id: "permission-scope:interview-notes:read-once",
  label: "访谈记录只读范围",
  source: "review",
  sensitivity: "sensitive",
};

const taskEvidence: EvidenceRef = {
  id: "task-session:interview-notes",
  label: "客户访谈整理任务",
  source: "task",
  sensitivity: "local_private",
};

const boundaryEvidence: EvidenceRef = {
  id: "provider-route:local-model:interview-notes",
  label: "本次任务模型路由",
  source: "provider",
  sensitivity: "local_private",
};

const localBoundary: ProviderPrivacyBoundarySummary = {
  routeType: "local",
  externalTransmission: "not_sent",
  providerLabel: "本机模型服务",
  modelLabel: "本地分析模型",
  privacyLabel: "仅本机处理",
  risk: "none",
  localOnlyRequired: true,
  evidenceRefs: [boundaryEvidence],
};

function action(
  kind: "approve" | "reject" | "later" | "view_evidence",
  enabled = true,
  disabledReason?: string
): ReviewAction {
  const labels = {
    approve: "仅允许本次",
    reject: "拒绝",
    later: "稍后处理",
    view_evidence: "查看访问范围",
  } as const;
  return {
    id: `${reviewItemId}:${kind}`,
    label: labels[kind],
    kind,
    effect: kind === "view_evidence" ? "evidence_only" : "decision_only",
    enabled,
    ...(enabled ? {} : { disabledReason: disabledReason ?? "当前动作不可用。" }),
    requiresConfirmation: kind === "approve",
    targetReviewItemId: reviewItemId,
    expectedMaterializationStatusAfterDispatch: "not_applicable",
    completionProofAfterDispatch: false,
  } as ReviewAction;
}

function permissionItem(stage: FixtureStage, incomplete: boolean): ReviewItem {
  const status = stage === "running" ? "approved" : stage;
  const pendingDecision = ["pending", "deferred"].includes(status);
  const approveEnabled = pendingDecision && !incomplete;
  const allowedActions: ReviewAction[] = pendingDecision
    ? [
        action(
          "approve",
          approveEnabled,
          incomplete ? "缺少目标范围和有效期；不能批准。" : undefined
        ),
        action("reject"),
        action("later", status !== "deferred", "这项请求已经设为稍后处理。"),
        action("view_evidence"),
      ]
    : [action("view_evidence")];

  return {
    id: reviewItemId,
    type: "tool_permission",
    source: {
      kind: "proposal",
      proposalId: "proposal-interview-notes-read",
      proposalSource: "main_chat_agent",
      sourceDetail: "整理客户访谈任务的工具调用请求",
      runId: "run-interview-notes-01",
    },
    status,
    materializationStatus: "not_applicable",
    decisionContext: {
      reviewItemId,
      title: "读取本地客户访谈记录",
      summary: "为归纳下周需要验证的问题，任务请求只读访问这组访谈记录。",
      before: {
        kind: "text",
        summary: "任务暂停，尚未读取任何访谈文件",
        sensitivity: "local_private",
        truncated: false,
      },
      after: {
        kind: "text",
        summary: "仅允许同一读取动作访问指定目录一次",
        sensitivity: "sensitive",
        truncated: false,
      },
      reasonSummary: "任务需要比较三次访谈中的重复问题、分歧和未验证假设。",
      sourceSummary: "来自当前客户访谈整理任务，尚未执行工具动作。",
      impactSummary: "批准只建立一次精确授权，不会自动继续任务，也不会写入 LifeModel。",
      affectedObjectLabels: ["客户访谈记录（3 份 Markdown）"],
      expiresAt: incomplete ? undefined : "任务恢复后首次匹配动作完成时",
      permission: {
        status: incomplete ? "incomplete" : "ready",
        scopeKind: incomplete ? "unknown" : "action_bound",
        policy: incomplete ? "unknown" : "allow_once",
        toolLabel: "读取文件",
        toolName: "read_file",
        capabilityLabels: ["读取文本文件"],
        requestedTargetLabel: incomplete ? undefined : "OpenLife 工作区 / 访谈记录（只读）",
        resolvedTargetLabel: incomplete ? undefined : "3 个已解析的 Markdown 文件",
        purposeSummary: "只用于归纳本次访谈中的待验证问题。",
        scopeDigest: incomplete ? undefined : "sha256:fixture-scope-interview-notes",
        requestDigestKind: "input",
        requestDigest: "sha256:fixture-read-request",
        requestLengthBytes: 184,
        blockedRunId: "run-interview-notes-01",
        blockedStepIndex: 2,
        transmissionBoundary: {
          externalTransmission: "not_sent",
          summary: "文件内容留在本机，仅交给本地模型处理。",
          targetLabel: "本机模型服务",
          evidenceRefs: [boundaryEvidence],
        },
        expiresAt: incomplete ? undefined : "首次精确匹配后失效",
        revocationSummary: "拒绝会立即终止本次授权请求；批准后未使用的授权随任务结束失效。",
        missingFields: incomplete ? ["requestedTargetLabel", "scopeDigest", "expiresAt"] : [],
        evidenceRefs: [permissionEvidence, boundaryEvidence],
      },
      evidenceRefs: [taskEvidence, permissionEvidence],
    },
    allowedActions,
    risk: incomplete ? "unknown" : "medium",
    expiresAt: incomplete ? undefined : "任务恢复后首次匹配动作完成时",
    evidenceRefs: [taskEvidence, permissionEvidence, boundaryEvidence],
    targetRefs: [
      { id: taskId, kind: "task", label: "客户访谈整理任务" },
      { id: "workspace/interview-notes", kind: "external_resource", label: "访谈记录目录" },
    ],
    taskResumeRelation: {
      taskSessionId: taskId,
      resumeRequiresMaterialization: false,
      canRequestResume: status === "approved",
      resumeActionId: status === "approved" ? `${taskId}:resume` : undefined,
      blockedReason: status === "approved" ? undefined : "权限决定尚未批准，任务不能继续。",
    },
  };
}

function resumeControl(): TaskControl {
  return {
    id: `${taskId}:resume`,
    label: "继续任务",
    kind: "resume",
    effect: "task_resume_request",
    enabled: true,
    requiresConfirmation: true,
    targetTaskId: taskId,
    targetActionId: "action-read-interview-notes",
    completionProofAfterDispatch: false,
  };
}

function activeTask(stage: FixtureStage): TaskViewModelItem {
  const pending = stage === "pending" || stage === "deferred";
  const rejected = stage === "rejected";
  const running = stage === "running";
  return {
    canonicalTaskId: taskId,
    taskSessionId: taskId,
    relatedRunIds: ["run-interview-notes-01"],
    conversationId: "conversation-research-plan",
    title: "整理三次客户访谈，归纳下周要验证的问题",
    strategy: "react",
    lifecycleStatus: running ? "running" : rejected ? "blocked" : "waiting_permission",
    terminalDeliveryStatus: rejected ? "blocked" : "not_terminal",
    finalDeliveryEvidencePresent: false,
    pendingBlockers: pending
      ? ["读取本地访谈记录前需要你的决定；当前尚未访问文件。"]
      : rejected
        ? ["本次读取请求已拒绝；任务不会访问访谈记录。"]
        : [],
    pendingReviewItemRefs: pending
      ? [{ id: reviewItemId, kind: "review_item", label: "读取访谈记录的权限请求" }]
      : [],
    allowedControls: stage === "approved" ? [resumeControl()] : [],
    nextRecommendedControl: stage === "approved" ? "resume" : pending ? "open_review_item" : "none",
    latestResultPreview: {
      status: rejected ? "blocked" : "not_terminal",
      label: running ? "正在读取并比较访谈记录" : "任务停在文件读取之前",
      preview: running
        ? "已开始读取本地记录并提取重复问题；尚未形成最终结果。"
        : rejected
          ? "读取请求已拒绝，没有访问文件，也没有生成最终结果。"
          : stage === "approved"
            ? "一次性权限决定已经记录，等待你明确继续任务。"
            : "尚未读取文件，等待你核对访问范围。",
      evidenceRefs: [taskEvidence],
    },
    evidenceRefs: [taskEvidence],
    updatedAt: generatedAt,
  };
}

function taskSummary(items: TaskViewModelItem[]): TasksViewModel["summary"] {
  return {
    total: items.length,
    activeCount: items.filter(item =>
      ["running", "waiting_permission", "blocked"].includes(item.lifecycleStatus)
    ).length,
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
  };
}

function envelope<T>(
  data: T | null,
  status: ViewModelStatus,
  target: string
): ViewModelEnvelope<T> {
  const unavailable = status === "error";
  const stale = status === "stale";
  return {
    data: unavailable ? null : data,
    status,
    lastUpdatedAt: stale ? "2026-07-17T09:30:00.000Z" : generatedAt,
    source: "backend-readmodel",
    evidenceRefs: unavailable ? [] : [taskEvidence],
    warnings: unavailable
      ? [
          {
            code: `fixture.${target}.load_failed`,
            message: `${target} fixture is unavailable.`,
            severity: "error",
            evidenceRefs: [],
          },
        ]
      : stale
        ? [
            {
              code: `fixture.${target}.stale`,
              message: `${target} fixture is stale.`,
              severity: "warning",
              evidenceRefs: [taskEvidence],
            },
          ]
        : [],
    actions: { primary: [], review: [], debugOnly: [] },
  };
}

function readStatus(id: Phase4dFixtureId): ViewModelStatus {
  if (id === "fixture-error") return "error";
  if (id === "fixture-stale") return "stale";
  if (id === "fixture-empty") return "empty";
  return "ready";
}

function buildSnapshot(
  id: Phase4dFixtureId,
  stage: FixtureStage,
  durableStage: DurableFixtureStage,
  providerReviewStage: ProviderTestFixtureStage | null
): GovernedActionSnapshot {
  const status = readStatus(id);
  const empty = id === "fixture-empty";
  const incomplete = id === "fixture-incomplete-permission";
  const item = permissionItem(stage, incomplete);
  const durableItem = durableReviewItem(durableStage);
  const providerItem = providerReviewStage ? providerTestReviewItem(providerReviewStage) : null;
  const task = activeTask(stage);
  const reviewItems = empty ? [] : [item, durableItem, ...(providerItem ? [providerItem] : [])];
  const workspace: WorkspaceViewModel = {
    ...(empty ? {} : { activeTask: task }),
    recentTaskRefs: empty ? [] : [{ id: taskId, kind: "task", label: task.title }],
    pendingReviewItems: !empty && (stage === "pending" || stage === "deferred") ? [item] : [],
    activity: empty
      ? []
      : [
          {
            id: "activity:interview-plan",
            kind: "plan",
            label: "已确定整理步骤",
            summary: "先读取三份访谈记录，再比较重复问题和分歧。",
            status: "recorded",
            evidenceRefs: [taskEvidence],
            occurredAt: "2026-07-20T09:28:00.000Z",
          },
          {
            id: "activity:interview-permission",
            kind: "permission_request",
            label:
              stage === "approved" || stage === "running" ? "一次性权限已记录" : "等待访问决定",
            summary:
              stage === "running"
                ? "任务已经恢复并开始本地读取；尚未完成。"
                : stage === "approved"
                  ? "任务仍等待明确的恢复请求。"
                  : "工具动作尚未执行。",
            status:
              stage === "running"
                ? "recorded"
                : stage === "rejected"
                  ? "blocked"
                  : stage === "approved"
                    ? "recorded"
                    : "waiting_decision",
            evidenceRefs: [permissionEvidence],
            occurredAt: generatedAt,
          },
        ],
    providerPrivacyBoundarySummary: localBoundary,
    activityRedactionState: "metadata_only",
    sourceRefs: [taskEvidence, permissionEvidence],
    contractLimitations: [
      "Fixture activity contains metadata only.",
      "Approval and task resume require separate refreshed read-model proof.",
    ],
  };
  const review: ReviewCenterViewModel = {
    batches: empty
      ? []
      : [
          {
            id: "batch:interview-notes-permission",
            domain: "tool_permission",
            sessionId: taskId,
            itemIds: [reviewItemId],
            actionRequiredCount: ["pending", "deferred"].includes(item.status) ? 1 : 0,
            highestRisk: item.risk,
          },
          {
            id: "batch:lifemodel-focus-preference",
            domain: "life_model",
            itemIds: [durableReviewItemId],
            actionRequiredCount: ["pending", "edited", "deferred"].includes(durableItem.status)
              ? 1
              : 0,
            highestRisk: durableItem.risk,
          },
          ...(providerItem
            ? [
                {
                  id: "batch:provider-connection-test",
                  domain: "tool_permission" as const,
                  itemIds: [providerTestReviewItemId],
                  actionRequiredCount: ["pending", "edited", "deferred"].includes(
                    providerItem.status
                  )
                    ? 1
                    : 0,
                  highestRisk: providerItem.risk,
                },
              ]
            : []),
        ],
    items: reviewItems,
    summary: {
      total: reviewItems.length,
      actionRequiredCount: reviewItems.filter(candidate =>
        ["pending", "edited", "deferred"].includes(candidate.status)
      ).length,
      blockedActionCount: reviewItems.filter(candidate =>
        candidate.allowedActions.some(candidateAction => !candidateAction.enabled)
      ).length,
      byStatus: empty
        ? {}
        : reviewItems.reduce<Record<string, number>>((counts, candidate) => {
            counts[candidate.status] = (counts[candidate.status] ?? 0) + 1;
            return counts;
          }, {}),
      byRisk: empty
        ? {}
        : reviewItems.reduce<Record<string, number>>((counts, candidate) => {
            counts[candidate.risk] = (counts[candidate.risk] ?? 0) + 1;
            return counts;
          }, {}),
      byMaterializationStatus: empty
        ? {}
        : reviewItems.reduce<Record<string, number>>((counts, candidate) => {
            counts[candidate.materializationStatus] =
              (counts[candidate.materializationStatus] ?? 0) + 1;
            return counts;
          }, {}),
    },
  };
  const taskItems = empty ? [] : [task];
  const tasks: TasksViewModel = {
    items: taskItems,
    summary: taskSummary(taskItems),
    sourceRefs: empty ? [] : [taskEvidence],
    contractLimitations: [
      "Command return is not task completion proof.",
      "Only refreshed exact task identity may confirm resume.",
    ],
  };

  return {
    workspaceEnvelope: envelope(workspace, status, "workspace"),
    reviewEnvelope: envelope(review, status, "review"),
    tasksEnvelope: envelope(tasks, status, "tasks"),
    diagnostics: [
      {
        id: "workspace_view_model",
        status: status === "error" ? "failed" : "loaded",
        message: status === "error" ? "Static error fixture." : undefined,
      },
      {
        id: "review_center_view_model",
        status: status === "error" ? "failed" : "loaded",
        message: status === "error" ? "Static error fixture." : undefined,
      },
      {
        id: "tasks_view_model",
        status: status === "error" ? "failed" : "loaded",
        message: status === "error" ? "Static error fixture." : undefined,
      },
    ],
  };
}

export function phase4dJourneyFixtureDataSource(id: Phase4dFixtureId): Phase4dJourneyDataSource {
  const readOnly = phase4dFixtureDataSource(id);
  const settingsFixture = createPhase4dSettingsFixture(id);
  let stage: FixtureStage = "pending";
  let durableStage = initialDurableFixtureStage(id);

  return {
    ...readOnly,
    ...settingsFixture.dataSource,
    async load() {
      const providerItem = settingsFixture.currentReviewItem();
      return buildSnapshot(
        id,
        stage,
        durableStage,
        providerItem ? (providerItem.status as ProviderTestFixtureStage) : null
      );
    },
    async loadDurableTruth() {
      return buildDurableFixtureSnapshot(id, durableStage);
    },
    async dispatchReviewAction(reviewAction) {
      if (readStatus(id) !== "ready") throw new Error("fixture_review_read_model_not_ready");
      if (settingsFixture.dispatchReviewAction(reviewAction)) return;
      if (
        reviewAction.targetReviewItemId !== reviewItemId &&
        reviewAction.targetReviewItemId !== durableReviewItemId
      ) {
        throw new Error("fixture_review_target_mismatch");
      }
      if (!reviewAction.enabled)
        throw new Error(reviewAction.disabledReason || "fixture_action_disabled");
      if (reviewAction.targetReviewItemId === durableReviewItemId) {
        if (reviewAction.kind === "approve") durableStage = "approved_not_applied";
        else if (reviewAction.kind === "reject") durableStage = "rejected";
        else if (reviewAction.kind === "later") durableStage = "deferred";
        else throw new Error("fixture_durable_review_action_unsupported");
      } else if (reviewAction.kind === "approve") stage = "approved";
      else if (reviewAction.kind === "reject") stage = "rejected";
      else if (reviewAction.kind === "later") stage = "deferred";
      else throw new Error("fixture_review_action_unsupported");
    },
    async resumeTask(control) {
      if (readStatus(id) !== "ready") throw new Error("fixture_workspace_read_model_not_ready");
      if (
        stage !== "approved" ||
        control.id !== `${taskId}:resume` ||
        control.targetTaskId !== taskId ||
        control.effect !== "task_resume_request"
      ) {
        throw new Error("fixture_resume_control_mismatch");
      }
      stage = "running";
    },
  };
}
