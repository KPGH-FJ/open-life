import type {
  EvidenceRef,
  ChatSession,
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
import type { ChatMessage } from "@/types";
import type { WorkbenchDataSource, WorkbenchSnapshot } from "@/app/workbenchDataSource";
import type { ConversationDataSource } from "@/features/conversation/conversationDataSource";
import type { PersonalIntelligenceDataSource } from "@/features/personalIntelligence/personalIntelligenceDataSource";
import type { SettingsDataSource } from "@/features/settings/settingsDataSource";
import {
  buildPersonalIntelligenceFixtureSnapshot,
  durableReviewItem,
  durableReviewItemId,
  initialDurableFixtureStage,
  type DurableFixtureStage,
} from "./personalIntelligence";
import type { WorkbenchFixtureId } from "./readOnly";
import {
  createSettingsFixture,
  providerTestReviewItem,
  providerTestReviewItemId,
  type ProviderTestFixtureStage,
} from "./settings";

export type WorkbenchFixtureDataSource = WorkbenchDataSource &
  PersonalIntelligenceDataSource &
  SettingsDataSource &
  ConversationDataSource;

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
  };
}

function activeTask(stage: FixtureStage): TaskViewModelItem {
  const pending = stage === "pending" || stage === "deferred";
  const rejected = stage === "rejected";
  const running = stage === "running";
  return {
    canonicalTaskId: taskId,
    relatedRunIds: ["run-interview-notes-01"],
    conversationId: "conversation-research-plan",
    title: "整理三次客户访谈，归纳下周要验证的问题",
    latestRunProvenance: {
      runId: "run-interview-notes-01",
      turnId: "turn-interview-notes-01",
      turnStatus: running ? "running" : rejected ? "failed" : "blocked",
      providerProfileId: "provider-profile:local-analysis",
      providerId: "ollama",
      modelId: "qwen2.5:14b",
      endpointClass: "local",
      executionMode: "scoped_agent",
      projectId: "project-customer-research",
      projectName: "客户研究",
      projectRevision: 3,
      projectScopeDigest: "sha256:project-customer-research-r3",
    },
    lifecycleStatus: running ? "running" : rejected ? "blocked" : "waiting_permission",
    terminalDeliveryStatus: rejected ? "blocked" : "not_terminal",
    finalDeliveryEvidencePresent: false,
    completionLimitations: [],
    items: [],
    workPlan: {
      revision: 1,
      steps: [
        {
          id: "read-notes",
          kind: "read_workspace_file",
          required: true,
          dependsOn: [],
        },
        {
          id: "deliver",
          kind: "deliver_result",
          required: true,
          dependsOn: ["read-notes"],
        },
      ],
      completion: { resultKind: "answer", requiresVerification: false },
      budgetPolicy: {
        maxPlanAttempts: 2,
        maxProviderAttempts: 8,
        maxToolAttempts: 8,
        maxTotalItems: 32,
      },
    },
    artifacts: [],
    pendingBlockers: pending
      ? ["读取本地访谈记录前需要你的决定；当前尚未访问文件。"]
      : rejected
        ? ["本次读取请求已拒绝；任务不会访问访谈记录。"]
        : [],
    pendingReviewItemRefs: pending
      ? [{ id: reviewItemId, kind: "review_item", label: "读取访谈记录的权限请求" }]
      : [],
    allowedControls: [],
    nextRecommendedControl: pending ? "open_review_item" : "none",
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
    needsAttentionCount: items.filter(item => item.needsAttention).length,
    activeCount: items.filter(item =>
      ["running", "waiting_review", "waiting_permission", "blocked"].includes(item.lifecycleStatus)
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

function readStatus(id: WorkbenchFixtureId): ViewModelStatus {
  if (id === "fixture-error") return "error";
  if (id === "fixture-stale") return "stale";
  if (id === "fixture-empty") return "empty";
  return "ready";
}

function buildSnapshot(
  id: WorkbenchFixtureId,
  stage: FixtureStage,
  durableStage: DurableFixtureStage,
  providerReviewStage: ProviderTestFixtureStage | null
): WorkbenchSnapshot {
  const status = readStatus(id);
  const empty = id === "fixture-empty";
  const incomplete = id === "fixture-incomplete-permission";
  const item = permissionItem(stage, incomplete);
  const durableItem = durableReviewItem(durableStage);
  const providerItem = providerReviewStage ? providerTestReviewItem(providerReviewStage) : null;
  const task = activeTask(stage);
  const reviewItems = empty ? [] : [item, durableItem, ...(providerItem ? [providerItem] : [])];
  const workspace: WorkspaceViewModel = {
    selectedConversationId: empty ? undefined : task.conversationId,
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
    activityRedactionState: "metadata_only",
    sourceRefs: [taskEvidence, permissionEvidence],
    contractLimitations: [
      "Fixture activity contains metadata only.",
      "Approval and task resume require separate refreshed read-model proof.",
    ],
  };
  const review: ReviewCenterViewModel = {
    batches: [
      ...(!empty
        ? [
            {
              id: "batch:interview-notes-permission",
              domain: "tool_permission" as const,
              sessionId: taskId,
              itemIds: [reviewItemId],
              actionRequiredCount: ["pending", "deferred"].includes(item.status) ? 1 : 0,
              highestRisk: item.risk,
            },
            {
              id: "batch:lifemodel-focus-preference",
              domain: "life_model" as const,
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
      byStatus: reviewItems.reduce<Record<string, number>>((counts, candidate) => {
        counts[candidate.status] = (counts[candidate.status] ?? 0) + 1;
        return counts;
      }, {}),
      byRisk: reviewItems.reduce<Record<string, number>>((counts, candidate) => {
        counts[candidate.risk] = (counts[candidate.risk] ?? 0) + 1;
        return counts;
      }, {}),
      byMaterializationStatus: reviewItems.reduce<Record<string, number>>((counts, candidate) => {
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
    capturedAt: generatedAt,
    workspaceEnvelope: envelope(workspace, status, "workspace"),
    reviewEnvelope: envelope(
      review,
      status === "empty" && reviewItems.length > 0 ? "ready" : status,
      "review"
    ),
    tasksEnvelope: envelope(tasks, status, "tasks"),
    boundaryEnvelope: envelope(localBoundary, status, "provider_boundary"),
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
      {
        id: "provider_privacy_boundary",
        status: status === "error" ? "failed" : "loaded",
        message: status === "error" ? "Static error fixture." : undefined,
      },
    ],
  };
}

export function workbenchFixtureDataSource(id: WorkbenchFixtureId): WorkbenchFixtureDataSource {
  const settingsFixture = createSettingsFixture(id);
  let stage: FixtureStage = "pending";
  let durableStage = initialDurableFixtureStage(id);
  let sessions: ChatSession[] = [
    {
      session_id: "conversation-research-plan",
      title: "整理客户访谈",
      created_at: "2026-07-20T09:20:00.000Z",
      updated_at: generatedAt,
    },
  ];
  let archivedSessions: ChatSession[] = [];
  const histories = new Map<string, ChatMessage[]>([
    [
      "conversation-research-plan",
      [
        { role: "user", content: "帮我整理这三次访谈，找出下周最值得验证的问题。" },
        {
          role: "assistant",
          content: "我已经拆分整理步骤。读取指定记录前需要你确认一次性访问范围。",
        },
      ],
    ],
  ]);
  async function applyTaskControl(control: TaskControl): Promise<void> {
    if (readStatus(id) !== "ready") throw new Error("fixture_workspace_read_model_not_ready");
    if (control.targetTaskId !== taskId) throw new Error("fixture_task_control_target_mismatch");
    throw new Error(`fixture_task_control_unsupported:${control.kind}`);
  }
  return {
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
    async stopRun() {},
    async loadPersonalIntelligence() {
      return buildPersonalIntelligenceFixtureSnapshot(id, durableStage);
    },
    async draftLifeModelChange() {
      return "fixture-lifemodel-v2-change-proposal";
    },
    async draftLegacyLifeModelMigration() {
      return "fixture-lifemodel-v2-migration-proposal";
    },
    async draftLifeModelRollback() {
      return "fixture-lifemodel-v2-rollback-proposal";
    },
    async draftLifeModelExport() {
      return "fixture-lifemodel-v2-export-proposal";
    },
    async confirmLifeModelLearningCandidate() {},
    async stageLifeModelLearningCandidate() {
      return "fixture-lifemodel-learning-proposal";
    },
    async deleteLifeModelLearningCandidate() {},
    async rejectLifeModelLearningCandidate() {},
    async pauseLifeModelLearningSuggestionClass() {},
    async correctMemory() {},
    async archiveMemory() {},
    async restoreMemory() {},
    async rollbackMemory() {},
    async privacyEraseMemory() {},
    async loadConversation(conversationId) {
      if (readStatus(id) === "error") throw new Error("fixture_conversation_store_unavailable");
      const conversations = sessions.map(session => ({ ...session }));
      const selectedConversationId =
        conversationId && histories.has(conversationId)
          ? conversationId
          : (conversations[0]?.session_id ?? null);
      return {
        status: conversations.length > 0 ? "ready" : "empty",
        conversations,
        archivedConversations: archivedSessions.map(session => ({ ...session })),
        projects: [],
        selectedProjectId: null,
        selectedConversationId,
        globalMemoryEnabled: true,
        selectedMemoryMode: "use_and_learn",
        messages: selectedConversationId
          ? (histories.get(selectedConversationId) ?? []).map((message, index) => ({
              ...message,
              turnId: `fixture-turn-${index}`,
              attachmentsStatus:
                message.role === "user" ? ("ready" as const) : ("not_applicable" as const),
              attachments: [],
            }))
          : [],
        latestTurn: null,
        providerStatus: "ready",
        providerProfiles: [],
        selectedProviderProfileId: null,
        providerErrorCode: null,
        workStatus: "available",
      };
    },
    async createSession(sessionId, title) {
      if (readStatus(id) !== "ready") throw new Error("fixture_workspace_read_model_not_ready");
      const timestamp = "2026-07-20T09:35:00.000Z";
      sessions = [
        { session_id: sessionId, title, created_at: timestamp, updated_at: timestamp },
        ...sessions,
      ];
      histories.set(sessionId, []);
    },
    async renameSession(sessionId, title) {
      const current = sessions.find(session => session.session_id === sessionId);
      if (!current) throw new Error("fixture_conversation_session_missing");
      sessions = sessions.map(session =>
        session.session_id === sessionId ? { ...session, title } : session
      );
    },
    async archiveSession(sessionId) {
      const current = sessions.find(session => session.session_id === sessionId);
      if (!current) throw new Error("fixture_conversation_session_missing");
      const messageCount = histories.get(sessionId)?.length ?? 0;
      sessions = sessions.filter(session => session.session_id !== sessionId);
      archivedSessions = [
        {
          ...current,
          status: "archived",
          turnCount: messageCount > 0 ? 1 : 0,
          itemCount: messageCount,
          taskReferenceCount: sessionId === "conversation-research-plan" ? 1 : 0,
          activeTaskCount: 0,
          allowedControls:
            messageCount === 0 && sessionId !== "conversation-research-plan"
              ? ["restore", "delete"]
              : ["restore"],
          blockerCodes:
            messageCount > 0
              ? ["conversation_delete_history_present"]
              : sessionId === "conversation-research-plan"
                ? ["conversation_delete_task_history_present"]
                : [],
        },
        ...archivedSessions,
      ];
    },
    async restoreSession(sessionId) {
      const current = archivedSessions.find(session => session.session_id === sessionId);
      if (!current) throw new Error("fixture_conversation_session_missing");
      archivedSessions = archivedSessions.filter(session => session.session_id !== sessionId);
      sessions = [{ ...current, status: "active", allowedControls: ["archive"] }, ...sessions];
    },
    async deleteSession(sessionId) {
      const current = archivedSessions.find(session => session.session_id === sessionId);
      if (!current?.allowedControls?.includes("delete")) {
        throw new Error("fixture_conversation_delete_not_eligible");
      }
      archivedSessions = archivedSessions.filter(session => session.session_id !== sessionId);
      histories.delete(sessionId);
    },
    async pickResources(importOperationId, turnOperationId) {
      return {
        cancelled: false,
        receipt: {
          operationId: importOperationId,
          messageId: turnOperationId,
          resources: [
            {
              resourceId: "4a006c47-67ee-4421-9f84-736f37926090",
              bindingId: "fixture-resource-binding",
              filename: "访谈记录.md",
              digest: "fixture-resource-digest",
              byteCount: 2048,
              chunkCount: 1,
              reusedExisting: false,
              eventId: "fixture-resource-event",
            },
          ],
          committedAt: generatedAt,
        },
      };
    },
    async detachResource(operationId, turnOperationId, resourceId) {
      return {
        operationId,
        messageId: turnOperationId,
        resourceId,
        bindingRemoved: true,
        resourceDeleted: true,
        eventId: "fixture-resource-detach-event",
        committedAt: generatedAt,
      };
    },
    async streamTurn(sessionId, messages, options, events) {
      if (readStatus(id) !== "ready") throw new Error("fixture_workspace_read_model_not_ready");
      if (!histories.has(sessionId)) throw new Error("fixture_conversation_session_missing");
      const reply = "我会先把目标拆成可核对的步骤；需要访问或写入时会单独请求你的决定。";
      events.onStart({
        session_id: sessionId,
        operation_id: options.operationId,
        conversation_id: sessionId,
        turn_id: options.operationId,
      });
      events.onChunk({
        session_id: sessionId,
        operation_id: options.operationId,
        conversation_id: sessionId,
        turn_id: options.operationId,
        chunk: reply,
      });
      histories.set(sessionId, [...messages, { role: "assistant", content: reply }]);
      return {
        session_id: sessionId,
        operation_id: options.operationId,
        conversation_id: sessionId,
        turn_id: options.operationId,
        reply,
        status: "completed",
        blockers: [],
      } as Awaited<ReturnType<ConversationDataSource["streamTurn"]>>;
    },
    async cancelChatTurn() {
      return { status: "cancelled" };
    },
    async dispatchReviewAction(reviewAction) {
      if (readStatus(id) !== "ready") {
        throw new Error("fixture_review_read_model_not_ready");
      }
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
      } else if (reviewAction.kind === "approve") stage = "running";
      else if (reviewAction.kind === "reject") stage = "rejected";
      else if (reviewAction.kind === "later") stage = "deferred";
      else throw new Error("fixture_review_action_unsupported");
    },
    async editLifeModelLearningProposal() {},
    async dispatchTaskControl(control) {
      await applyTaskControl(control);
    },
    async requestArtifactUndo() {},
    async requestTaskArtifactUndo() {
      return { failures: [] };
    },
    async reviseArtifact() {},
    async openArtifactResult() {},
    async exportArtifactResult() {
      return null;
    },
  };
}
