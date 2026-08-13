import { vi } from "vitest";
import type { LifeModel, ChatMessage, DailyGoal, StateHistoryEntry, StateAlert } from "@/types";
import type {
  LifeModelViewModel,
  MemoryViewModel,
  ProviderPrivacyBoundarySummary,
  TasksViewModel,
  ViewModelEnvelope,
  WorkspaceViewModel,
} from "@/tauri";

export const mockLifeModel: LifeModel = {
  metadata: {
    version: "0.1.0",
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
    author: "test",
  },
  identity: {
    name: "测试用户",
    values: [
      { name: "健康", weight: 0.8, description: "保持身体健康" },
      { name: "学习", weight: 0.7, description: "持续学习成长" },
    ],
    personality_traits: [
      { trait_name: "外向", score: 0.8 },
      { trait_name: "乐观", score: 0.9 },
    ],
    life_philosophy: "活在当下",
    mission_statement: "成为更好的自己",
    role_definition: {
      primary_role: "开发者",
      secondary_roles: ["家庭成员"],
      responsibilities: ["工作", "家庭"],
      boundaries: ["不加班"],
    },
    voice_style: {
      formality: "casual",
      tone_descriptors: ["友好"],
      vocabulary_preference: "简洁",
      emoji_usage: "often",
    },
  },
  goals: {
    short_term: [
      {
        name: "完成项目",
        priority: 1,
        status: "in_progress",
        milestones: [
          {
            name: "设计阶段",
            target_date: "2024-12-01",
            status: "completed",
            description: "完成设计",
          },
        ],
        description: "完成当前项目",
        progress: 0.5,
        related_memories: [],
      },
    ],
    medium_term: [],
    long_term: [],
    life_goals: [],
    daily: [{ name: "早起", done: false, time_block: { start: "07:00", end: "08:00" } }],
    progress: 0.5,
    related_memories: [],
  },
  capabilities: {
    skills: [
      { name: "编程", proficiency: 0.8, description: "软件开发" },
      { name: "写作", proficiency: 0.6, description: "技术写作" },
    ],
    resources: [
      { name: "MacBook", resource_type: "设备", description: "工作电脑", availability: "" },
    ],
    networks: [],
    tools: [],
    knowledge_domains: [{ domain: "AI", level: 7, description: "人工智能领域" }],
  },
  state: {
    current_focus: "工作",
    health_status: { physical: "良好", mental: "稳定", energy_level: 8 },
    emotional_state: { current_mood: "happy", stress_level: 2, fulfillment_score: 8 },
    recent_reflections: [],
    open_questions: [],
    focus_areas: ["工作", "学习"],
    recent_events: [],
    habit_streaks: [{ name: "阅读", streak_days: 5 }],
    custom_dimensions: [{ name: "专注度", unit: "%", current_value: 75, alert_days: 3 }],
    alerts: [],
  },
  relationships: { inner_circle: [], mentors: [], collaborators: [] },
  preferences: {
    work_hours: { preferred_start: "09:00", preferred_end: "18:00", timezone: "Asia/Shanghai" },
    peak_energy_time: "",
    communication_style: "",
    learning_style: "",
    decision_making_style: "",
  },
  evolution_rules: [],
};

export function createEmptyLifeModel(): LifeModel {
  return {
    metadata: {
      version: "0.1.0",
      created_at: "",
      updated_at: "",
      author: "",
    },
    identity: {
      name: "",
      birth_date: undefined,
      values: [],
      personality_traits: [],
      life_philosophy: "",
      mission_statement: "",
      role_definition: {
        primary_role: "",
        secondary_roles: [],
        responsibilities: [],
        boundaries: [],
      },
      voice_style: {
        formality: "neutral",
        tone_descriptors: [],
        vocabulary_preference: "",
        emoji_usage: "never",
      },
    },
    goals: {
      short_term: [],
      medium_term: [],
      long_term: [],
      life_goals: [],
      daily: [],
      progress: 0,
      related_memories: [],
    },
    capabilities: {
      skills: [],
      resources: [],
      networks: [],
      tools: [],
      knowledge_domains: [],
    },
    state: {
      current_focus: "",
      health_status: {
        physical: "",
        mental: "",
        energy_level: 0,
      },
      emotional_state: {
        current_mood: "",
        stress_level: 0,
        fulfillment_score: 0,
      },
      recent_reflections: [],
      open_questions: [],
      focus_areas: [],
      recent_events: [],
      habit_streaks: [],
      custom_dimensions: [],
      alerts: [],
    },
    relationships: {
      inner_circle: [],
      mentors: [],
      collaborators: [],
    },
    preferences: {
      work_hours: {
        preferred_start: "",
        preferred_end: "",
        timezone: "",
      },
      peak_energy_time: "",
      communication_style: "",
      learning_style: "",
      decision_making_style: "",
    },
    evolution_rules: [],
  };
}

export function createMockLifeModelViewModelEnvelope(
  overrides: Partial<ViewModelEnvelope<LifeModelViewModel>> = {}
): ViewModelEnvelope<LifeModelViewModel> {
  const now = new Date().toISOString();
  const base: ViewModelEnvelope<LifeModelViewModel> = {
    data: {
      truthMode: "unknown",
      canonicalSummary: null,
      versionHistory: [],
      legacyMigrationPreview: null,
      trustQualityState: {
        readiness: "usable_with_limits",
        warningRefs: [],
        ownerStatus: "PARTIAL",
      },
      pendingUpdateCounts: {
        candidate: 1,
        pendingReview: 1,
        approvedNotApplied: 0,
        failedMaterialization: 0,
        ownerStatus: "PARTIAL",
      },
      provenanceRefs: [],
      candidateChanges: [
        {
          changeRef: {
            id: "proposal:proposal-life-model-1",
            kind: "proposal",
            label: "LifeModel update",
          },
          title: "Life Model 更新",
          changeKind: "update",
          affectedDimensionIds: ["goals"],
          reviewItemRefs: [
            { id: "proposal-life-model-1", kind: "review_item", label: "Review item" },
          ],
          evidenceRefs: [
            {
              id: "proposal:proposal-life-model-1",
              label: "Proposal record",
              source: "review",
              sensitivity: "local_private",
            },
          ],
          decisionStatus: "pending",
        },
      ],
      materializedChanges: [],
      manualOverrideState: {
        active: false,
        blockedReason:
          "Whole-model manual saves are unavailable; use the proposal-first review flow.",
        draftRef: null,
        saveAction: null,
        reviewItemRefs: [],
        evidenceRefs: [],
        ownerStatus: "PARTIAL",
      },
      relatedReviewItemRefs: [
        { id: "proposal-life-model-1", kind: "review_item", label: "Review item" },
      ],
      memoryLinkage: {
        linkedMemoryCount: 12,
        candidateMemoryCount: 0,
        materializedMemoryCount: 0,
        conflictCount: 0,
        memoryRefs: [],
        evidenceRefs: [],
        linkageStatus: "partial",
        tierSummary: { total: 12, tier1: 5, tier2: 4, tier3: 3, archived: 0 },
        ownerStatus: "PHASE_2_REQUIRED",
      },
      learning: {
        available: true,
        activeCount: 0,
        candidates: [],
      },
      sourceRefs: [
        {
          id: "projection:diagnostics",
          label: "LifeStateProjection",
          source: "backend-readmodel",
          sensitivity: "local_private",
        },
      ],
      contractLimitations: [
        "Accepted proposal decisions remain approved-not-applied unless backend evidence proves applied.",
      ],
    },
    status: "ready",
    lastUpdatedAt: now,
    source: "backend-readmodel",
    evidenceRefs: [],
    warnings: [],
    actions: { primary: [] },
  };

  return {
    ...base,
    ...overrides,
    data:
      overrides.data === undefined
        ? base.data
        : overrides.data === null
          ? null
          : { ...base.data!, ...overrides.data },
  };
}

export function createMockTasksViewModelEnvelope(
  overrides: Partial<ViewModelEnvelope<TasksViewModel>> = {}
): ViewModelEnvelope<TasksViewModel> {
  const now = new Date().toISOString();
  const base: ViewModelEnvelope<TasksViewModel> = {
    data: {
      items: [
        {
          canonicalTaskId: "mainchat_task_mock",
          taskSessionId: "mainchat_task_mock",
          relatedRunIds: ["run_mainchat_mock"],
          conversationId: "session-1",
          title: "mock goal",
          strategy: "direct_answer",
          lifecycleStatus: "completed_needs_evidence",
          terminalDeliveryStatus: "missing_final_delivery_evidence",
          finalDeliveryEvidencePresent: false,
          pendingBlockers: ["terminal_no_resume"],
          pendingReviewItemRefs: [],
          items: [],
          artifacts: [],
          allowedControls: [
            {
              id: "mainchat_task_mock:open_trace",
              label: "Open trace",
              kind: "open_trace",
              effect: "evidence_only",
              enabled: true,
              targetTaskId: "mainchat_task_mock",
              completionProofAfterDispatch: false,
            },
          ],
          nextRecommendedControl: "open_trace",
          latestResultPreview: {
            status: "missing_final_delivery_evidence",
            label: "missing final delivery evidence",
            preview: "mock complete",
            evidenceRefs: [],
          },
          evidenceRefs: [],
          updatedAt: now,
        },
      ],
      summary: {
        total: 1,
        activeCount: 0,
        waitingReviewCount: 0,
        waitingPermissionCount: 0,
        blockedCount: 0,
        pendingReviewCount: 0,
        completedCount: 0,
        completedNeedsEvidenceCount: 1,
        failedCount: 0,
        cancelledCount: 0,
        byLifecycleStatus: { completed_needs_evidence: 1 },
      },
      sourceRefs: [],
      contractLimitations: [
        "Resume, retry, cancel, and refresh controls are request eligibility only.",
      ],
    },
    status: "ready",
    lastUpdatedAt: now,
    source: "backend-readmodel",
    evidenceRefs: [],
    warnings: [],
    actions: { primary: [] },
  };

  return {
    ...base,
    ...overrides,
    data:
      overrides.data === undefined
        ? base.data
        : overrides.data === null
          ? null
          : { ...base.data!, ...overrides.data },
  };
}

export function createMockWorkspaceViewModelEnvelope(
  overrides: Partial<ViewModelEnvelope<WorkspaceViewModel>> = {}
): ViewModelEnvelope<WorkspaceViewModel> {
  const now = new Date().toISOString();
  const base: ViewModelEnvelope<WorkspaceViewModel> = {
    data: {
      activeTask: {
        canonicalTaskId: "mainchat_task_mock",
        taskSessionId: "mainchat_task_mock",
        relatedRunIds: ["run_mainchat_mock"],
        conversationId: "conversation_mock",
        title: "mock goal",
        strategy: "react",
        lifecycleStatus: "running",
        terminalDeliveryStatus: "not_terminal",
        finalDeliveryEvidencePresent: false,
        pendingBlockers: [],
        pendingReviewItemRefs: [],
        items: [],
        artifacts: [],
        allowedControls: [],
        nextRecommendedControl: "open_trace",
        evidenceRefs: [],
        updatedAt: now,
      },
      recentTaskRefs: [
        {
          id: "mainchat_task_mock",
          kind: "task",
          label: "mock goal",
          href: "/runs/run_mainchat_mock",
        },
      ],
      pendingReviewItems: [],
      activity: [
        {
          id: "event_mock",
          kind: "action",
          label: "Action requested",
          summary: "action_state_recorded",
          status: "recorded",
          evidenceRefs: [],
          occurredAt: now,
        },
      ],
      providerPrivacyBoundarySummary: {
        routeType: "unknown",
        externalTransmission: "unknown",
        providerLabel: "provider unknown",
        modelLabel: "model unknown",
        privacyLabel: "privacy boundary unknown",
        risk: "unknown",
        localOnlyRequired: false,
        evidenceRefs: [],
      },
      activityRedactionState: "metadata_only",
      sourceRefs: [],
      contractLimitations: ["Workspace activity is metadata-only."],
    },
    status: "ready",
    lastUpdatedAt: now,
    source: "backend-readmodel",
    evidenceRefs: [],
    warnings: [],
    actions: { primary: [] },
  };

  return {
    ...base,
    ...overrides,
    data:
      overrides.data === undefined
        ? base.data
        : overrides.data === null
          ? null
          : { ...base.data!, ...overrides.data },
  };
}

export function createMockProviderPrivacyBoundarySummaryEnvelope(
  overrides: Partial<ViewModelEnvelope<ProviderPrivacyBoundarySummary>> = {}
): ViewModelEnvelope<ProviderPrivacyBoundarySummary> {
  const now = new Date().toISOString();
  const base: ViewModelEnvelope<ProviderPrivacyBoundarySummary> = {
    data: {
      routeType: "local",
      externalTransmission: "not_sent",
      providerLabel: "local model",
      modelLabel: "llama2",
      privacyLabel: "LocalOnly route; external transmission not required",
      risk: "low",
      localOnlyRequired: true,
      evidenceRefs: [],
    },
    status: "ready",
    lastUpdatedAt: now,
    source: "backend-readmodel",
    evidenceRefs: [],
    warnings: [],
    actions: { primary: [] },
  };

  return {
    ...base,
    ...overrides,
    data:
      overrides.data === undefined
        ? base.data
        : overrides.data === null
          ? null
          : { ...base.data!, ...overrides.data },
  };
}

export function createMockMemoryViewModelEnvelope(
  overrides: Partial<ViewModelEnvelope<MemoryViewModel>> = {}
): ViewModelEnvelope<MemoryViewModel> {
  const now = new Date().toISOString();
  const base: ViewModelEnvelope<MemoryViewModel> = {
    data: {
      summary: {
        totalLifecycleRecords: 1,
        activeMemoryCount: 1,
        reviewRequiredCount: 0,
        materializedCount: 1,
        pendingMaterializationCount: 0,
        failedMaterializationCount: 0,
        rolledBackCount: 0,
        archivedVectorCount: 0,
        conflictCount: 0,
        tierSummary: { total: 1, tier1: 1, tier2: 0, tier3: 0, archived: 0 },
      },
      lifecycleSummary: {
        candidateCount: 0,
        pendingReviewCount: 0,
        editedPendingReviewCount: 0,
        acceptedCount: 0,
        confirmedCount: 1,
        pendingMaterializationCount: 0,
        materializedCount: 1,
        materializationFailedCount: 0,
        rejectedCount: 0,
        deferredCount: 0,
        supersededCount: 0,
        rolledBackCount: 0,
        expiredCount: 0,
        archivedCount: 0,
        byStatus: { materialized: 1 },
        byMaterializationStatus: { materialized: 1 },
      },
      laneSummaries: [
        {
          lane: "semantic_fact_preference",
          label: "Semantic facts and preferences",
          totalCount: 1,
          activeCount: 1,
          candidateCount: 0,
          pendingReviewCount: 0,
          confirmedCount: 1,
          materializedCount: 1,
          rolledBackCount: 0,
          archivedCount: 0,
          reviewItemRefs: [],
          evidenceRefs: [],
        },
      ],
      recentMemoryRefs: [{ id: "memory:mock", kind: "memory", label: "preference · materialized" }],
      reviewItemRefs: [],
      lifeModelLinkage: {
        linkedMemoryCount: 1,
        candidateMemoryCount: 0,
        materializedMemoryCount: 1,
        conflictCount: 0,
        boundaryMemoryCount: 0,
        linkageStatus: "partial",
        memoryRefs: [],
        evidenceRefs: [],
      },
      items: [
        {
          memoryId: "memory:mock",
          content: "User prefers concise answers.",
          scope: "global",
          category: "preference",
          status: "materialized",
          materializationStatus: "materialized",
          recallState: "active",
          sensitivity: "internal",
          whyRemembered: "The user approved this reviewed Memory proposal.",
          recallExplanation: "The current task and scope must match before recall.",
          acceptedAt: now,
          evidenceIds: ["message:mock"],
          sourceRefs: [],
          privacyErased: false,
          canCorrect: true,
          canStopRecall: true,
          canArchive: true,
          canRestore: false,
          canRollback: true,
          canPrivacyErase: true,
        },
      ],
      sourceRefs: [],
      contractLimitations: [
        "Vector tier counts are supporting storage telemetry; lifecycle materialization status remains the product memory authority.",
      ],
    },
    status: "ready",
    lastUpdatedAt: now,
    source: "backend-readmodel",
    evidenceRefs: [],
    warnings: [],
    actions: { primary: [] },
  };

  return {
    ...base,
    ...overrides,
    data:
      overrides.data === undefined
        ? base.data
        : overrides.data === null
          ? null
          : { ...base.data!, ...overrides.data },
  };
}

export const mockDailyGoals: DailyGoal[] = [
  { name: "早起", done: false, time_block: { start: "07:00", end: "08:00" } },
  { name: "运动", done: true },
];

export const mockStateAlerts: StateAlert[] = [
  {
    dimension_name: "专注度",
    level: "warning",
    message: "专注度低于阈值",
    triggered_at: new Date().toISOString(),
  },
];

export const mockStateHistory: StateHistoryEntry[] = [
  {
    id: 1,
    dimension_name: "专注度",
    value: 70,
    unit: "%",
    note: "上午工作",
    recorded_at: new Date().toISOString(),
  },
  {
    id: 2,
    dimension_name: "专注度",
    value: 75,
    unit: "%",
    note: "下午工作",
    recorded_at: new Date(Date.now() - 86400000).toISOString(),
  },
];

export const mockChatSessions: Array<{ session_id: string; title: string; updated_at: string }> = [
  { session_id: "session-1", title: "会话 1", updated_at: new Date().toISOString() },
  {
    session_id: "session-2",
    title: "会话 2",
    updated_at: new Date(Date.now() - 3600000).toISOString(),
  },
];

export const mockChatMessages: ChatMessage[] = [
  { role: "user", content: "你好" },
  { role: "assistant", content: "你好！我是 OpenLife。" },
];

export const mockPreviewAgentRun = {
  id: "run-preview-1",
  taskId: "task-preview-1",
  sessionId: "session-preview",
  status: "completed",
  kind: "conversation",
  generatedProposals: [],
  actions: [],
  observations: [],
  reasoningStrategy: "multi_strategy_preview",
  reasoningTrace: {
    strategy_result: {
      previewRuntime: "multi_strategy",
      strategyKind: "react",
      payloadKind: "react",
      governanceDecisionKind: "allow",
      riskLevel: "low",
      reasonCode: "default_react",
      hasPolicyContext: true,
      warnings: [],
      proposalIds: [],
      planStepCount: 0,
      planStepStatuses: [],
      blocked: false,
      metadataSafe: true,
    },
  },
  outputPreview: "Multi-strategy preview: react / allow",
  startedAt: new Date().toISOString(),
};

function mockStage5PreflightFixture() {
  return {
    reportKind: "main_chat_stage5_release_debug_preflight",
    schemaVersion: "stage5-preflight-v1",
    createdAt: "2026-06-20T00:00:00Z",
    build: {
      commit: null,
      branch: null,
      appVersion: "0.1.0",
      buildTimestamp: null,
      dirtyState: null,
      blockers: ["build_commit_unavailable"],
    },
    provider: {
      provider: "openai",
      model: "gpt-4.1-mini",
      routeType: "default_preflight_no_invocation",
      keyPresent: false,
      networkOptIn: false,
      liveProviderInvocationAllowed: false,
      liveProviderPreflightStatus: "blocked",
      blockers: ["provider_api_key_missing"],
    },
    scheduler: {
      schedulerType: "scripted_eval",
      scriptedProviderResponsePresent: true,
      preferLocal: false,
      localModelConfigured: false,
    },
    workspace: {
      rootDigest: "sha256:workspace",
      safePathCount: 1,
      safePathsDigest: "sha256:safe-paths",
      safePathsConfigured: true,
      blockers: [],
    },
    mcp: {
      registryAvailable: true,
      manifestCount: 1,
      readCandidateCount: 1,
      blockers: [],
    },
    database: {
      memoryStoreAvailable: true,
      agentRunStoreAvailable: true,
      taskSessionStoreAvailable: true,
      actionQueueStoreAvailable: true,
      proposalStoreAvailable: true,
      memoryLifecycleStoreAvailable: true,
      blockers: [],
    },
    stage2Readiness: {
      recommendation: "not_ready_for_limited_internal_trial",
      blockers: ["stage2_manual_dogfood_evidence_missing"],
    },
    finalAcceptance: {
      recommendation: "not_final_completion_ready",
      blockers: ["live_provider_generation_not_executed"],
    },
    failure: {
      class: "environment_preflight_failure",
      severity: "p1",
      scope: "environment",
      recoverability: "needs_environment_fix",
      recoveryRecommendation: "Fix local provider, workspace, MCP, or store configuration.",
      evidence: ["provider_api_key_missing"],
    },
    externalProviderInvokedByDefault: false,
    modelInvoked: false,
    directWritesExecuted: false,
    metadataSafe: true,
    blockers: ["provider_api_key_missing"],
  };
}

export const mockInvoke = vi.fn(<T>(cmd: string, args?: Record<string, any>): Promise<T> => {
  const _args = args;
  switch (cmd) {
    case "get_config":
      return Promise.resolve({
        llm: {
          provider: "deepseek",
          openai_base: "https://api.deepseek.com",
          openai_key: "",
          embedding_model: "text-embedding-3-small",
          chat_model: "deepseek-chat",
          embedding_enabled: false,
        },
        prefer_local_model: false,
        local_model: "llama3",
      } as T);
    case "get_life_model_view_model":
      return Promise.resolve(createMockLifeModelViewModelEnvelope() as T);
    case "delete_lifemodel_learning_candidate":
      return Promise.resolve({
        candidateId: _args?.candidateId ?? _args?.candidate_id ?? "",
        deleted: true,
        proposalDeleted: false,
        canonicalLifeModelChanged: false,
      } as T);
    case "draft_lifemodel_v2_change":
      return Promise.resolve({
        proposalId: "proposal:lifemodel-v2-change:mock",
        status: "review_required",
        baseVersion: _args?.request?.baseVersion ?? null,
        baseDocumentDigest: _args?.request?.baseDocumentDigest ?? null,
        resultDocumentDigest: "sha256:result",
        operationCount: 1,
      } as T);
    case "draft_lifemodel_v2_rollback":
      return Promise.resolve({
        proposalId: "proposal:lifemodel-v2-rollback:mock",
        status: "review_required",
        baseVersion: _args?.request?.baseVersion ?? null,
        baseDocumentDigest: _args?.request?.baseDocumentDigest ?? null,
        resultDocumentDigest: _args?.request?.targetDocumentDigest ?? null,
        operationCount: 1,
      } as T);
    case "draft_lifemodel_v2_export":
      return Promise.resolve({
        proposalId: "proposal:lifemodel-v2-export:mock",
        status: "review_required",
        baseVersion: _args?.request?.modelVersion ?? null,
        baseDocumentDigest: _args?.request?.documentDigest ?? null,
        resultDocumentDigest: null,
        operationCount: 0,
      } as T);
    case "get_memory_view_model":
      return Promise.resolve(createMockMemoryViewModelEnvelope() as T);
    case "get_markdown_memory_view_model":
      return Promise.resolve({
        roots: [
          { scope: "workspace", configured: false, rootPath: null, status: "unconfigured" },
          { scope: "project", configured: false, rootPath: null, status: "unconfigured" },
        ],
        files: [],
        totalCharCount: 0,
        truncated: false,
        sourceRule: "explicit roots only",
      } as T);
    case "select_markdown_memory_root":
      return Promise.resolve({
        cancelled: true,
        scope: _args?.scope ?? "project",
        selectedPath: null,
      } as T);
    case "draft_markdown_memory_file_proposal":
      return Promise.resolve({
        proposalId: "proposal:markdown-memory:mock",
        scope: _args?.request?.scope ?? "project",
        relativePath: _args?.request?.relativePath ?? "MEMORY.md",
        operation: "write",
        status: "review_required",
      } as T);
    case "deactivate_markdown_memory_file_proposal":
      return Promise.resolve({
        proposalId: "proposal:markdown-memory:mock-deactivate",
        scope: _args?.request?.scope ?? "project",
        relativePath: _args?.request?.relativePath ?? "MEMORY.md",
        operation: "deactivate",
        status: "review_required",
      } as T);
    case "get_provider_privacy_boundary_summary":
      return Promise.resolve(createMockProviderPrivacyBoundarySummaryEnvelope() as T);
    case "get_tasks_view_model":
      return Promise.resolve(createMockTasksViewModelEnvelope() as T);
    case "get_workspace_view_model":
      return Promise.resolve(createMockWorkspaceViewModelEnvelope() as T);
    case "get_daily_goals":
      return Promise.resolve(mockDailyGoals as T);
    case "get_state_alerts":
      return Promise.resolve(mockStateAlerts as T);
    case "get_state_history":
      return Promise.resolve(mockStateHistory as T);
    case "get_conversation_view_model": {
      const requested = _args?.conversationId ?? _args?.conversation_id;
      const selected =
        mockChatSessions.find(session => session.session_id === requested)?.session_id ??
        mockChatSessions[0]?.session_id;
      return Promise.resolve({
        status: mockChatSessions.length ? "ready" : "empty",
        conversations: mockChatSessions,
        selectedConversationId: selected,
        messages: selected ? mockChatMessages : [],
        providerStatus: "ready",
        providerProfiles: [
          {
            profileId: "provider-profile:mock",
            providerId: "deepseek",
            modelId: "deepseek-chat",
            endpointClass: "cloud",
            selected: true,
          },
        ],
        selectedProviderProfileId: "provider-profile:mock",
        providerErrorCode: null,
        latestTurn: null,
        workStatus: "reconstructing",
      } as T);
    }
    case "list_provider_transmission_history":
      return Promise.resolve([] as T);
    case "get_main_chat_agent_task_state":
      return Promise.resolve({
        session: {
          id: _args?.taskSessionId ?? _args?.task_session_id ?? "mainchat_task_mock",
          chatSessionId: "session-1",
          userGoal: "mock goal",
          selectedStrategy: "direct_answer",
          status: "completed",
          currentPlanSummary: undefined,
          actionQueueIds: [],
          pendingBlockers: [],
          contextSnapshotRefs: [],
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
          finalSummary: "mock complete",
        },
        actions: [],
        transcript: [],
        pendingApprovalCount: 0,
        activeToolCount: 0,
        canResume: false,
        canCancel: false,
        canRetry: false,
      } as T);
    case "list_main_chat_agent_tasks":
      return Promise.resolve([] as T);
    case "get_main_chat_agent_task_detail":
    case "refresh_main_chat_agent_task_context":
      return Promise.resolve({
        taskSession: {
          id: _args?.taskSessionId ?? _args?.task_session_id ?? "mainchat_task_mock",
          chatSessionId: "session-1",
          userGoal: "mock goal",
          selectedStrategy: "direct_answer",
          status: "completed",
          currentPlanSummary: undefined,
          actionQueueIds: [],
          pendingBlockers: [],
          contextSnapshotRefs: [],
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
          finalSummary: "mock complete",
        },
        actions: [],
        transcript: [],
        proposals: [],
        blockers: [],
        finalDelivery: null,
        continuityDiagnostics: {
          staleContext: false,
          missingActionEvidence: false,
          permissionScopeMismatch: false,
          terminalNoResume: true,
          providerUnavailable: false,
          toolUnavailable: false,
          requiresUserDecision: false,
          selectedSkillContextDigestMismatch: false,
          planRevisionMismatch: false,
          reasonCodes: ["terminal_no_resume"],
          automaticReplayAllowed: false,
        },
        allowedControls: ["open_trace"],
        nextRecommendedControl: "open_trace",
        lastSafeResumePoint: null,
        contextDigest: "bytes:2 hash:sha256:mock",
        selectedSkillDigest: null,
        toolManifestDigest: "bytes:2 hash:sha256:mock",
      } as T);
    case "list_main_chat_agent_events":
      return Promise.resolve([] as T);
    case "list_main_chat_skills":
      return Promise.resolve([
        {
          skillId: "evidence_review",
          name: "Phase E Review",
          source: "workspace:skills/evidence_review/SKILL.md",
          scope: "session",
          description: "Review Main Chat Skill/Tool evidence.",
          riskLevel: "low",
          available: true,
          selected: false,
          instructionDigest:
            "bytes:80 hash:sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
          sourceKind: "workspace",
          lastUsedAt: null,
        },
      ] as T);
    case "get_main_chat_skill_detail":
      return Promise.resolve({
        skillId: _args?.skillId ?? _args?.skill_id ?? "evidence_review",
        manifest: {
          name: "Phase E Review",
          source: "workspace:skills/evidence_review/SKILL.md",
          sourceKind: "workspace",
          available: true,
        },
        boundedInstructionsPreview: "Use Phase E skill evidence as bounded context only.",
        allowedTools: ["builtin_echo"],
        disallowedTools: ["email.send"],
        policyNotes: ["Selected SKILL.md is bounded context, not authority."],
        requiredPermissions: [],
        evidenceDigest:
          "bytes:120 hash:sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        redactionSummary: "bounded_preview_no_secrets",
        lastModifiedAt: "2026-06-17T00:00:00.000Z",
      } as T);
    case "select_main_chat_skill":
      return Promise.resolve({
        sessionId: _args?.sessionId ?? _args?.session_id ?? "session-1",
        selectedSkillId: _args?.skillId ?? _args?.skill_id ?? "evidence_review",
        selectedSkillDigest:
          "bytes:80 hash:sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        selectionReason: "user_selected_local_skill",
        boundedInstructionsPreview: "Use Phase E skill evidence as bounded context only.",
        evidenceDigest:
          "bytes:120 hash:sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        policyNotes: ["Selected SKILL.md is bounded context, not authority."],
        includedAsBoundedContextOnly: true,
        unselectedSkillsInjected: false,
        controls: ["clear_skill"],
      } as T);
    case "clear_main_chat_skill":
      return Promise.resolve({
        sessionId: _args?.sessionId ?? _args?.session_id ?? "session-1",
        selectedSkillId: null,
        selectedSkillDigest: null,
        selectionReason: "user_cleared_local_skill",
        boundedInstructionsPreview: "",
        evidenceDigest:
          "bytes:34 hash:sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
        policyNotes: ["Next task context has no selected skill."],
        includedAsBoundedContextOnly: false,
        unselectedSkillsInjected: false,
        controls: ["select_skill"],
      } as T);
    case "list_main_chat_tool_candidates":
      return Promise.resolve({
        taskSessionId: _args?.taskSessionId ?? _args?.task_session_id ?? null,
        candidates: [
          {
            candidateId: "builtin_echo",
            toolName: "builtin_echo",
            source: "builtin",
            capabilityLabels: ["read"],
            riskLevel: "low",
            selectionReason: "manifest_default_order",
            policyDecision: "allow",
            requiresPermission: false,
            candidateDigest:
              "bytes:88 hash:sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
            linkedActionId: null,
          },
        ],
        blockedTools: [
          {
            toolName: "email.send",
            reasonCode: "write_like_tool_blocked",
            policyDecision: "permission_required",
            requiresPermission: true,
            blockerId: "blocker-email-send",
          },
        ],
        failureRecovery: null,
        evidenceDigest:
          "bytes:142 hash:sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        controls: [],
      } as T);
    case "get_main_chat_agent_state_snapshot":
      return Promise.resolve({
        task: {
          taskId: _args?.taskSessionId ?? _args?.task_session_id ?? "mainchat_task_mock",
          runId: "run_mainchat_mock",
          title: "mock goal",
          strategy: "direct_answer",
          status: "completed",
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
          actionIds: [],
          observationIds: [],
          proposalIds: [],
          blockerIds: [],
          pendingControlIds: [],
        },
        route: {
          strategy: "direct_answer",
          reason: "mock route",
          confidence: 1,
        },
        context: [],
        provider: undefined,
        plan: undefined,
        actions: [],
        observations: [],
        blockers: [],
        proposals: [],
        finalDelivery: undefined,
        diagnostics: [],
        sequence: 1,
        emittedAt: new Date().toISOString(),
        events: [],
      } as T);
    case "resume_main_chat_agent_task":
    case "cancel_main_chat_agent_task":
    case "retry_main_chat_agent_action":
      return Promise.resolve({
        session: null,
        actions: [],
        transcript: [],
        pendingApprovalCount: 0,
        activeToolCount: 0,
        canResume: false,
        canCancel: false,
        canRetry: false,
      } as T);
    case "run_main_chat_agent_execution_v1_eval_gate":
      return Promise.resolve({
        reportKind: "main_chat_agent_execution_v1_eval_gate",
        runtimeEval: {
          totalCases: 100,
          runtimeExecutedCaseCount: 100,
          deterministicStubCaseCount: 0,
          passedCases: 100,
          failedCases: 0,
          silentWriteCount: 0,
          finalCompletionReady: false,
          finalCompletionBlockers: [
            "live_provider_generation_not_executed",
            "provider_backed_web_mcp_agent_loop_not_executed",
            "provider_live_proposal_permission_not_executed",
          ],
          failures: [],
        },
        acceptance: {
          ready: false,
          status: "blocked",
          blockers: ["command_surface_cases_below_24", "live_provider_generation_not_executed"],
          requiredEvidence: [
            "core_100_case_runtime_eval",
            "send_stream_command_surface_eval",
            "live_provider_generation",
            "provider_backed_web_mcp_agent_loop",
            "provider_backed_web_agent_loop",
            "provider_backed_mcp_agent_loop",
            "provider_live_proposal_permission",
          ],
          runtimeGateReady: false,
          commandSurfaceGateReady: false,
          liveProviderGateReady: false,
          directWritesExecuted: false,
        },
        liveProviderPreflight: {
          ready: false,
          status: "blocked",
          provider: "openai",
          blockers: ["explicit_live_eval_required", "provider_api_key_missing"],
          requiredEvidence: [
            "live_provider_generation",
            "provider_backed_web_mcp_agent_loop",
            "provider_backed_web_agent_loop",
            "provider_backed_mcp_agent_loop",
            "provider_live_proposal_permission",
          ],
          liveProviderInvocationAllowed: false,
          modelInvoked: false,
          directWritesExecuted: false,
        },
        commandSurfaceGateExecuted: false,
        liveProviderAttempted: false,
        migrationPermission: false,
        metadataSafe: true,
        noExternalProviderInvocation: true,
        noAppStoreWrites: true,
        metadataSafeSummary: {
          reportKind: "main_chat_agent_execution_v1_eval_gate",
          runtimeTotalCases: 100,
          acceptanceReady: false,
          liveProviderPreflightReady: false,
          liveProviderPreflightStatus: "blocked",
          liveProviderPreflightProvider: "openai",
          liveProviderPreflightBlockers: [
            "explicit_live_eval_required",
            "provider_api_key_missing",
          ],
          liveProviderPreflightRequiredEvidence: [
            "live_provider_generation",
            "provider_backed_web_mcp_agent_loop",
            "provider_backed_web_agent_loop",
            "provider_backed_mcp_agent_loop",
            "provider_live_proposal_permission",
          ],
          liveProviderPreflightInvocationAllowed: false,
          liveProviderPreflightModelInvoked: false,
          liveProviderPreflightDirectWritesExecuted: false,
          metadataSafe: true,
        },
      } as T);
    case "run_main_chat_runtime_contract_gate":
      return Promise.resolve({
        totalScenarioCount: 93,
        defaultDeterministicScenarioCount: 92,
        readinessSemantics: "full_deterministic_productization_v1_runtime_ready",
        runtimeExecutionScope:
          "default_deterministic_scenarios_runtime_backed_external_live_excluded",
        executedScenarioCount: 92,
        passedScenarioCount: 81,
        expectedBlockerScenarioCount: 11,
        failedScenarioCount: 0,
        externalLiveExcludedCount: 1,
        runtimePayloadSnapshotEventGatePassed: true,
        runtimeRequiredGroupCount: 92,
        runtimeRequiredGroupPassedCount: 92,
        representativeRuntimeGroupCount: 0,
        representativeRuntimeGroupPassedCount: 0,
        fullDeterministicRuntimeScenarioCount: 92,
        fullDeterministicRuntimeScenarioExecutedCount: 92,
        runtimeRequiredGroupEvidence: [],
        eventSemantics:
          "durable_replayable_delta_events_available_snapshot_backfill_excluded_from_live_credit",
        finalReadinessReady: true,
        fullProductizationV1Complete: true,
        futureWork: [],
        routeCounts: {},
        unsupportedScenarios: [],
        failedScenarios: [],
        blockers: [],
      } as T);
    case "run_main_chat_stage3_execution_ux_report":
      return Promise.resolve({
        reportKind: "main_chat_stage3_execution_ux",
        schemaVersion: "stage3-execution-ux-v1",
        dataPath:
          "Main Chat send/stream -> AgentIngress / strategy route -> AgentTaskSession / ActionQueue / ExecutionTranscript / Main Chat event stream -> MainChatAgentStateSnapshot -> AgentControlPlane",
        totalScenarioCount: 13,
        passedScenarioCount: 13,
        failedScenarioCount: 0,
        blockedScenarioCount: 0,
        executionFirstRequiredIds: [
          "UX3-02",
          "UX3-03",
          "UX3-04",
          "UX3-06",
          "UX3-09",
          "UX3-11",
          "UX3-12",
        ],
        executionFirstPassedIds: [
          "UX3-02",
          "UX3-03",
          "UX3-04",
          "UX3-06",
          "UX3-09",
          "UX3-11",
          "UX3-12",
        ],
        executionFirstClaimValid: true,
        readyForLimitedInternalTrial: false,
        readinessRecommendation: "not_ready_for_limited_internal_trial",
        stage2ReadinessPreserved:
          "stage2_readiness_remains_fail_closed_without_manual_dogfood_and_current_commit_live_evidence",
        nonGoals: [
          "manual_dogfood_rows_not_run_or_fabricated",
          "ready_for_limited_internal_trial_not_claimed",
        ],
        coverage: Array.from({ length: 13 }, (_, index) => ({
          scenarioId: `UX3-${String(index + 1).padStart(2, "0")}`,
          scenario: "covered Stage 3 scenario",
          status: "passed",
          evidence: ["runtime-backed evidence"],
          blockers: [],
        })),
        blockers: [],
      } as T);
    case "run_main_chat_stage4_memory_knowledge_report":
      return Promise.resolve({
        reportKind: "main_chat_stage4_memory_knowledge",
        schemaVersion: "stage4.v1",
        scenarioCount: 18,
        passedScenarioCount: 17,
        blockedScenarioCount: 1,
        notAReadinessGate: true,
        readinessClaim: false,
        stage2ReadinessPreserved: true,
        rows: Array.from({ length: 18 }, (_, index) => ({
          id: `MK4-${String(index + 1).padStart(2, "0")}`,
          scenario: "mock stage4 scenario",
          status: index === 17 ? "blocked" : "passed",
          evidenceIds: ["mock-stage4"],
          blockers: index === 17 ? ["managed_user_memory_write_lifecycle_not_yet_exercised"] : [],
        })),
        evidenceIds: ["mock-stage4"],
        blockers: ["managed_user_memory_write_lifecycle_not_yet_exercised"],
        activeMemoryIds: ["memory:active-1"],
        excludedMemoryIds: ["memory:rolled-back-1"],
        loadedKnowledgeAssetIds: ["knowledge:USER.md", "knowledge:MEMORY.md"],
        skippedKnowledgeAssetIds: ["knowledge:SOUL.md"],
        managedKnowledgeWriteAssetIds: [],
        managedKnowledgeWriteVersionIds: [],
        managedKnowledgeWriteAuditIds: [],
        managedKnowledgeRollbackSnapshotIds: [],
        directWriteCount: 0,
        confirmedKnowledgeWriteCount: 0,
        rollbackEventCount: 1,
      } as T);
    case "evaluate_main_chat_stage5_release_debug_preflight":
      return Promise.resolve(mockStage5PreflightFixture() as T);
    case "export_main_chat_agent_debug_bundle":
      return Promise.resolve({
        bundleId: "stage5-bundle-mock",
        schemaVersion: "stage5-debug-bundle-v1",
        createdAt: "2026-06-20T00:00:01Z",
        build: {
          commit: null,
          branch: null,
          appVersion: "0.1.0",
          buildTimestamp: null,
          dirtyState: null,
          blockers: ["build_commit_unavailable"],
        },
        environment: mockStage5PreflightFixture(),
        scenario: {
          scenarioId: args?.scenarioId ?? args?.scenario_id ?? "DBG5-04",
          reviewerId: args?.reviewerId ?? args?.reviewer_id ?? "internal-tester",
          status: null,
          notesDigest: null,
        },
        task: {
          chatSessionId: "chat-stage5-mock",
          taskSessionId: args?.taskSessionId ?? args?.task_session_id ?? "task-stage5-mock",
          runId: "run-stage5-mock",
          strategy: "direct_answer",
          status: "completed",
          userGoalDigest: "sha256:user-goal",
          transcriptEntryCount: 3,
          actionCount: 0,
          proposalCount: 0,
          blockerCount: 0,
          finalDeliveryId: "delivery-stage5-mock",
        },
        route: {
          routeType: "governed_direct_answer",
          provider: "scripted_eval",
          model: "mock-model",
          localOnly: false,
          liveProviderAttempted: false,
          providerEndpointKind: "local_synthetic",
        },
        timeline: [
          {
            itemId: "transcript-stage5-1",
            kind: "final_result",
            summaryPreview: "metadata-safe final result summary",
            metadataDigest: "sha256:transcript",
          },
        ],
        tools: {
          candidateCount: 0,
          selectedTool: null,
          actionType: null,
          targetDigest: null,
          policyDecision: null,
          observationCount: 0,
          actionStatuses: [],
        },
        context: {
          activeMemoryIds: [],
          excludedMemoryIds: [],
          knowledgeAssetIds: [],
          selectedSkillId: null,
          contextSourceDigests: [],
        },
        memory: {
          proposalIds: [],
          acceptedMemoryIds: [],
          rolledBackMemoryIds: [],
          managedKnowledgeVersionIds: [],
        },
        finalDelivery: {
          completedWorkCount: 1,
          durableChangeCount: 0,
          pendingUserActionCount: 0,
          skippedWorkCount: 0,
          blockerCount: 0,
          finalDeliveryDigest: "sha256:final-delivery",
        },
        failure: {
          class: "unknown_failure",
          severity: "p2",
          scope: "unknown",
          recoverability: "needs_developer_fix",
          recoveryRecommendation: "Triage with trace-backed evidence.",
          evidence: ["final_delivery_present"],
        },
        redaction: {
          mode: "metadata_safe",
          rawContentIncluded: false,
          secretsDetected: false,
          unsafeFieldCount: 0,
          unsafeFieldsDropped: [],
          previewLimit: 160,
          promptDigest: "sha256:prompt",
          responseDigest: "sha256:response",
          contextDigest: "sha256:context",
        },
        uiEvidence: args?.uiEvidence ?? args?.ui_evidence ?? null,
        artifact: {
          artifactId: "stage5-bundle-mock",
          artifactKind: "debug_bundle",
          schemaVersion: "stage5-debug-bundle-v1",
          createdAt: "2026-06-20T00:00:01Z",
          storageAlias: "stage5/debug_bundles/stage5-bundle-mock.json",
          digest: "sha256:bundle",
          byteSize: 4096,
        },
      } as T);
    case "create_main_chat_internal_issue_report":
      return Promise.resolve({
        reportId: "stage5-issue-mock",
        schemaVersion: "stage5-issue-report-v1",
        createdAt: "2026-06-20T00:00:02Z",
        scenarioId: args?.input?.scenarioId ?? "DBG5-19",
        reviewerId: args?.input?.reviewerId ?? "internal-tester",
        status: args?.input?.status ?? "fail",
        taskSessionId: args?.input?.taskSessionId ?? "task-stage5-mock",
        runId: args?.input?.runId ?? "run-stage5-mock",
        bundleId: args?.input?.bundleId ?? "stage5-bundle-mock",
        buildCommit: null,
        appVersion: "0.1.0",
        redactionMode: "metadata_safe",
        failureClass: args?.input?.failureClass ?? "unknown_failure",
        notesDigest: "sha256:notes",
        notesPreview: null,
        missingTaskRunReason: null,
        blockers: ["stage5_issue_notes_preview_redacted"],
        artifact: {
          artifactId: "stage5-issue-mock",
          artifactKind: "issue_report",
          schemaVersion: "stage5-issue-report-v1",
          createdAt: "2026-06-20T00:00:02Z",
          storageAlias: "stage5/issue_reports/stage5-issue-mock.json",
          digest: "sha256:issue",
          byteSize: 1024,
        },
      } as T);
    case "list_main_chat_debug_bundles":
      return Promise.resolve([
        {
          artifactId: "stage5-bundle-mock",
          artifactKind: "debug_bundle",
          schemaVersion: "stage5-debug-bundle-v1",
          createdAt: "2026-06-20T00:00:01Z",
          storageAlias: "stage5/debug_bundles/stage5-bundle-mock.json",
          digest: "sha256:bundle",
          byteSize: 4096,
        },
      ] as T);
    case "get_main_chat_debug_bundle":
      return mockInvoke("export_main_chat_agent_debug_bundle", args) as Promise<T>;
    case "delete_main_chat_debug_bundle":
      return Promise.resolve(true as T);
    case "list_main_chat_internal_issue_reports":
      return Promise.resolve([
        {
          artifactId: "stage5-issue-mock",
          artifactKind: "issue_report",
          schemaVersion: "stage5-issue-report-v1",
          createdAt: "2026-06-20T00:00:02Z",
          storageAlias: "stage5/issue_reports/stage5-issue-mock.json",
          digest: "sha256:issue",
          byteSize: 1024,
        },
      ] as T);
    case "get_main_chat_internal_issue_report":
      return mockInvoke("create_main_chat_internal_issue_report", {
        input: {
          scenarioId: "DBG5-19",
          reviewerId: "internal-tester",
          status: "fail",
          taskSessionId: "task-stage5-mock",
          runId: "run-stage5-mock",
          bundleId: args?.reportId ?? args?.report_id ?? "stage5-bundle-mock",
          failureClass: "unknown_failure",
        },
      }) as Promise<T>;
    case "delete_main_chat_internal_issue_report":
      return Promise.resolve(true as T);
    case "run_main_chat_stage5_release_debug_report":
      return Promise.resolve({
        reportKind: "main_chat_stage5_release_debug",
        schemaVersion: "stage5-release-debug-v1",
        scenarioCount: 24,
        passedScenarioCount: 20,
        blockedScenarioCount: 4,
        notAReadinessGate: true,
        readinessClaim: false,
        rows: Array.from({ length: 24 }, (_, index) => ({
          id: `DBG5-${String(index + 1).padStart(2, "0")}`,
          scenario: "mock stage5 scenario",
          status: index < 20 ? "passed" : "blocked",
          evidenceIds: ["stage5_release_debug_report"],
          bundleIds: ["stage5-bundle-mock"],
          issueArtifactIds: index === 18 ? ["stage5-issue-mock"] : [],
          blockers: index < 20 ? [] : ["stage5_scenario_evidence_missing"],
        })),
        evidenceIds: ["stage5_release_debug_report"],
        blockers: ["DBG5-24:stage5_scenario_evidence_missing"],
        build: {
          commit: null,
          branch: null,
          appVersion: "0.1.0",
          buildTimestamp: null,
          dirtyState: null,
          blockers: ["build_commit_unavailable"],
        },
        preflightSummary: mockStage5PreflightFixture(),
        bundleIds: ["stage5-bundle-mock"],
        issueArtifactIds: ["stage5-issue-mock"],
        artifactStorageSummary: [
          {
            artifactId: "stage5-bundle-mock",
            artifactKind: "debug_bundle",
            schemaVersion: "stage5-debug-bundle-v1",
            createdAt: "2026-06-20T00:00:01Z",
            storageAlias: "stage5/debug_bundles/stage5-bundle-mock.json",
            digest: "sha256:bundle",
            byteSize: 4096,
          },
        ],
        redactionSummary: {
          mode: "metadata_safe",
          rawContentIncluded: false,
          secretsDetected: false,
          unsafeFieldCount: 0,
          unsafeFieldsDropped: [],
          previewLimit: 160,
          promptDigest: null,
          responseDigest: null,
          contextDigest: "sha256:stage5-report",
        },
        managedKnowledgeEval: {
          isolatedEvalAppState: true,
          tempWorkspace: true,
          realWorkspaceWriteExecuted: false,
          userWriteCompleted: true,
          memoryRollbackCompleted: true,
          managedKnowledgeWriteVersionIds: ["knowledge_version:stage5-mock"],
          managedKnowledgeAuditIds: ["knowledge_audit:stage5-mock"],
          rollbackSnapshotIds: ["snapshot:stage5-mock"],
          evidenceIds: ["stage5_isolated_managed_knowledge_eval"],
          blockers: [],
        },
        stage2ReadinessPreserved: true,
      } as T);
    case "run_main_chat_agent_product_maturity_v2_event_gate":
      return Promise.resolve({
        scenarioCount: 8,
        defaultGateScenarioCount: 8,
        passedScenarioCount: 8,
        expectedBlockerCount: 0,
        ready: true,
        blockers: [],
        proofs: [
          {
            scenarioId: "EV-01",
            capabilityGroup: "event_delta_stream",
            passed: true,
            runtimeObjectCount: 2,
            emittedEventIds: ["mainchat_event:mock:1:route.selected:direct_answer:d1"],
            replayedEventIds: ["mainchat_event:mock:1:route.selected:direct_answer:d1"],
            emittedSequences: [1],
            replayedSequences: [1],
            uiState: ["subscribed", "receiving_event"],
            diagnostics: [],
          },
        ],
      } as T);
    case "run_main_chat_agent_product_maturity_v2_plan_gate":
      return Promise.resolve({
        scenarioCount: 10,
        defaultGateScenarioCount: 10,
        passedScenarioCount: 10,
        expectedBlockerCount: 3,
        ready: true,
        blockers: [],
        scenarios: [
          {
            id: "PI-01",
            capabilityGroup: "plan_interaction",
            prompt: "Plan this work before executing.",
            preconditions: ["none"],
            expectedRoute: "plan_execute",
            requiredRuntimeEvidence: ["plan.created", "step.created"],
            requiredUiState: ["plan_draft_visible"],
            requiredControls: ["confirm_plan", "edit_plan", "skip_step"],
            negativeAssertions: ["no_frontend_only_plan"],
            expectedOutcome: "pass",
            defaultGate: true,
          },
        ],
        proofs: [
          {
            scenarioId: "PI-01",
            passed: true,
            expectedBlocker: false,
            planId: "plan:mock-phase-c",
            revision: 1,
            stepIds: ["step-1"],
            eventTypes: ["plan.created", "step.created"],
            linkedActionIds: [],
            linkedObservationIds: [],
            linkedProposalIds: [],
            blockerIds: [],
            controls: ["confirm_plan", "edit_plan", "skip_step"],
            diagnostics: [],
          },
        ],
      } as T);
    case "run_main_chat_agent_product_maturity_v2_skills_gate":
      return Promise.resolve({
        scenarioCount: 8,
        defaultGateScenarioCount: 8,
        passedScenarioCount: 8,
        expectedBlockerCount: 2,
        ready: true,
        blockers: [],
        scenarios: [
          {
            id: "SK2-01",
            capabilityGroup: "skills_tools_surface",
            prompt: "Select a bounded local skill.",
            preconditions: ["local_skill_available"],
            expectedRoute: "direct_answer",
            requiredRuntimeEvidence: ["selected_skill.bounded_context"],
            requiredUiState: ["selected_skill_visible"],
            requiredControls: ["clear_skill"],
            negativeAssertions: ["skill_does_not_override_policy"],
            expectedOutcome: "pass",
            defaultGate: true,
          },
        ],
        proofs: [
          {
            scenarioId: "SK2-01",
            passed: true,
            expectedBlocker: false,
            runtimeObjectCount: 3,
            selectedSkillIds: ["evidence_review"],
            candidateIds: ["project_status.read"],
            blockerIds: [],
            actionIds: [],
            observationIds: [],
            controls: ["clear_skill"],
            runtimeEvidence: ["selected_skill.bounded_context"],
            uiState: ["selected_skill_visible"],
            negativeAssertions: ["skill_does_not_override_policy"],
            diagnostics: [],
          },
        ],
      } as T);
    case "run_main_chat_agent_product_maturity_v2_final_readiness_gate":
      return Promise.resolve({
        reportKind: "main_chat_agent_product_maturity_v2_final_readiness_gate",
        readinessSemantics:
          "phase_g_final_readiness_default_deterministic_live_product_opt_in_separate",
        defaultReadinessScope: "MR_EV_PI_LT2_SK2_deterministic_only",
        optInLiveReadinessScope: "LIVE_PROD_external_live_opt_in_only",
        finalReady: false,
        deterministicReady: true,
        optInLiveReady: false,
        finalReadinessStatus: "blocked_live_productization_not_ready",
        deterministicReadinessStatus: "ready",
        optInLiveReadinessStatus: "blocked",
        defaultDeterministicScenarioCount: 43,
        defaultLiveProdExcludedCount: 6,
        externalLiveScenarioCount: 6,
        defaultScenarioPassedCount: 33,
        defaultScenarioExpectedBlockerCount: 10,
        defaultScenarioFailedCount: 0,
        defaultScenarioBlockedCount: 0,
        externalLivePassedCount: 0,
        externalLiveBlockedCount: 6,
        externalLiveFailedCount: 0,
        phaseCounts: [
          {
            phaseId: "phase_a",
            phaseLabel: "Phase A Memory lifecycle",
            capabilityGroup: "memory_lifecycle",
            scenarioCount: 9,
            passed: 7,
            expectedBlocker: 2,
            failed: 0,
            blocked: 0,
            status: "ready",
            ready: true,
            defaultGate: true,
            optInOnly: false,
            blockers: [],
            supportedScenarios: ["MR-01", "MR-02", "MR-03", "MR-06", "MR-07", "MR-08"],
            blockedScenarios: ["MR-04", "MR-05"],
            unsupportedScenarios: [],
            futureScenarios: [],
          },
          {
            phaseId: "phase_f",
            phaseLabel: "Phase F External live product evidence",
            capabilityGroup: "external_live_productization",
            scenarioCount: 6,
            passed: 0,
            expectedBlocker: 0,
            failed: 0,
            blocked: 6,
            status: "blocked",
            ready: false,
            defaultGate: false,
            optInOnly: true,
            blockers: ["explicit_live_eval_required"],
            supportedScenarios: [],
            blockedScenarios: ["LIVE-PROD-01"],
            unsupportedScenarios: [],
            futureScenarios: [],
          },
        ],
        supportedScenarios: [
          {
            scenarioId: "MR-03",
            phaseId: "phase_a",
            capabilityGroup: "memory_lifecycle",
            status: "supported",
            reason: "passed",
          },
        ],
        blockedScenarios: [
          {
            scenarioId: "LIVE-PROD-01",
            phaseId: "phase_f",
            capabilityGroup: "external_live_productization",
            status: "blocked",
            reason: "explicit_live_eval_required",
          },
        ],
        unsupportedScenarios: [],
        futureScenarios: [],
        blockers: ["explicit_live_eval_required"],
        deterministicBlockers: [],
        optInLiveBlockers: ["explicit_live_eval_required"],
        directWritesExecuted: false,
        noSilentDurableWrites: true,
        defaultLiveProdExcluded: true,
      } as T);
    case "run_main_chat_agent_beta_v1_readiness_gate":
      return Promise.resolve({
        reportKind: "main_chat_agent_beta_v1_readiness_gate",
        readinessSemantics: "beta_v1_execution_first_default_deterministic_live_opt_in_separate",
        defaultReadinessScope: "beta_v1_default_deterministic_local_only",
        optInLiveReadinessScope: "beta_v1_external_live_opt_in_only",
        foundationInventoryExists: true,
        foundationInventoryItems: [
          {
            component: "Knowledge assets and context inventory",
            status: "partial",
            evidence: ["B27 inspection and B28 proposal-first edit evidence"],
            developmentDecision: "reuse minimum beta slice; broader manager deferred",
          },
        ],
        workstreams: [
          {
            workstreamId: "phase_5",
            label: "Capability Hardening",
            status: "ready",
            ready: true,
            evidence: ["structured readiness report and release notes"],
            blockers: [],
          },
        ],
        productMaturityPhaseCounts: [
          {
            phaseId: "phase_a",
            capabilityGroup: "memory_lifecycle",
            scenarioCount: 9,
            passed: 7,
            expectedBlocker: 2,
            failed: 0,
            blocked: 0,
            ready: true,
            optInOnly: false,
          },
        ],
        defaultReadinessStatus: "ready",
        defaultReady: true,
        optInLiveReady: false,
        externalLiveAttempted: false,
        defaultRealTaskScenarioCount: 28,
        defaultRealTaskPassedCount: 28,
        optInLiveRealTaskScenarioCount: 2,
        defaultExperienceRequiredStateCount: 11,
        defaultExperienceVerifiedStateCount: 11,
        productMaturityDefaultScenarioCount: 43,
        commandSurfaceTotalCases: 38,
        commandSurfaceFailedCases: 0,
        legacyFallbackCount: 0,
        silentDurableWriteCount: 0,
        noSilentDurableWrites: true,
        defaultBlockers: [],
        optInLiveBlockers: ["explicit_live_eval_required"],
        readinessDimensions: [
          {
            dimension: "Routing",
            status: "ready",
            optInOnly: false,
            evidence: ["governed task sessions and strategy routing"],
            blockers: [],
          },
          {
            dimension: "Live provider",
            status: "blocked_opt_in_not_attempted",
            optInOnly: true,
            evidence: ["external live evidence is opt-in and not run by default"],
            blockers: ["explicit_live_eval_required"],
          },
        ],
      } as T);
    case "run_main_chat_agent_stage2_readiness_gate":
      return Promise.resolve({
        reportKind: "main_chat_agent_stage2_readiness_gate",
        schemaVersion: "stage2-readiness-v1",
        runId: "stage2-readiness-mock",
        commit: "unknown",
        recommendation: "not_ready_for_limited_internal_trial",
        implementationStatus: "implementation_complete_for_stage2_mechanism",
        blockers: [
          "stage2_readiness_commit_missing",
          "stage2_manual_dogfood_evidence_missing",
          "stage2_live_provider_p0_evidence_missing",
        ],
        deterministicStage1Ready: true,
        betaFoundationReady: true,
        manualDogfood: {
          attempted: false,
          ready: false,
          reviewerCount: 0,
          requiredScenarioCount: 24,
          attemptedScenarioCount: 0,
          passedScenarioCount: 0,
          missingScenarioIds: ["S2-D01"],
          failedScenarioIds: ["S2-D01"],
          traceIdsPresent: false,
          artifactDigest: null,
          blockers: ["stage2_manual_dogfood_evidence_missing"],
        },
        liveProvider: {
          attempted: false,
          ready: false,
          provider: null,
          model: null,
          requiredScenarioCount: 10,
          passedScenarioCount: 0,
          failedScenarioIds: ["L2-L01"],
          modelInvokedCount: 0,
          mainChatInvokedCount: 0,
          localOrMockCreditRejected: 0,
          artifactDigest: null,
          blockers: ["stage2_live_provider_p0_evidence_missing"],
          scenarioPlans: [
            {
              scenarioId: "L2-L01",
              scenario: "direct_answer",
              scenarioSetup: "live_provider_enabled",
              requiredRuntimeEvidence: [
                "provider_model_identity",
                "model_invoked",
                "response_preview",
                "no_agent_loop_metadata",
              ],
              failClosedBlocker: "live_provider_generation_not_completed",
              executionSource: "existing_v1_live_harness",
              runnerStatus: "implemented",
            },
            {
              scenarioId: "L2-L02",
              scenario: "file_read_request",
              scenarioSetup: "seeded_workspace_file_or_missing_file_fixture",
              requiredRuntimeEvidence: ["file_action_or_blocker", "no_fake_observation"],
              failClosedBlocker: "live_provider_read_action_missing",
              executionSource: "stage2_live_file_read_runner",
              runnerStatus: "implemented",
            },
            {
              scenarioId: "L2-L03",
              scenario: "web_policy_blocker",
              scenarioSetup: "web_network_policy_disabled",
              requiredRuntimeEvidence: ["web_policy_blocker", "no_provider_backed_web_credit"],
              failClosedBlocker: "live_provider_web_policy_bypass",
              executionSource: "stage2_live_web_policy_runner",
              runnerStatus: "implemented",
            },
            {
              scenarioId: "L2-L04",
              scenario: "provider_backed_web_read",
              scenarioSetup: "governed_web_read_enabled",
              requiredRuntimeEvidence: [
                "selected_web_candidate",
                "action_status",
                "observation",
                "final_synthesis",
              ],
              failClosedBlocker: "provider_backed_web_agent_loop_not_executed",
              executionSource: "existing_v1_live_harness",
              runnerStatus: "implemented",
            },
            {
              scenarioId: "L2-L05",
              scenario: "registered_mcp_read",
              scenarioSetup: "two_bounded_read_only_mcp_candidates",
              requiredRuntimeEvidence: [
                "candidate_ids",
                "target_allowlist",
                "selected_rank",
                "observation",
              ],
              failClosedBlocker: "provider_backed_mcp_agent_loop_not_executed",
              executionSource: "existing_v1_live_harness",
              runnerStatus: "implemented",
            },
            {
              scenarioId: "L2-L06",
              scenario: "mcp_tool_permission_proposal",
              scenarioSetup: "permission_required_read_target",
              requiredRuntimeEvidence: [
                "tool_permission_proposal",
                "proposal_target",
                "selected_candidate",
                "no_read_success_overlap",
              ],
              failClosedBlocker: "provider_live_proposal_permission_not_executed",
              executionSource: "existing_v1_live_harness",
              runnerStatus: "implemented",
            },
            {
              scenarioId: "L2-L07",
              scenario: "multi_step_react",
              scenarioSetup: "two_safe_read_sources_available",
              requiredRuntimeEvidence: ["two_actions", "two_observations", "final_synthesis"],
              failClosedBlocker: "live_provider_multistep_observation_missing",
              executionSource: "stage2_live_multistep_react_runner",
              runnerStatus: "implemented",
            },
            {
              scenarioId: "L2-L08",
              scenario: "memory_proposal",
              scenarioSetup: "memory_proposal_enabled_no_auto_materialization",
              requiredRuntimeEvidence: [
                "proposal_id",
                "source_evidence",
                "no_memory_materialization",
              ],
              failClosedBlocker: "live_provider_memory_proposal_missing",
              executionSource: "stage2_live_memory_proposal_runner",
              runnerStatus: "implemented",
            },
            {
              scenarioId: "L2-L09",
              scenario: "permission_denial",
              scenarioSetup: "pending_safe_read_permission_denial",
              requiredRuntimeEvidence: ["denied_permission_state", "no_resumed_action"],
              failClosedBlocker: "live_provider_permission_denial_bypassed",
              executionSource: "stage2_live_permission_denial_runner",
              runnerStatus: "implemented",
            },
            {
              scenarioId: "L2-L10",
              scenario: "failure_recovery",
              scenarioSetup: "induced_bad_tool_or_safe_tool_failure",
              requiredRuntimeEvidence: [
                "blocker_reason",
                "retry_or_cancel_state",
                "no_fake_final_done",
              ],
              failClosedBlocker: "live_provider_failure_hidden",
              executionSource: "stage2_live_failure_recovery_runner",
              runnerStatus: "implemented",
            },
          ],
          scenarioReports: [
            {
              scenarioId: "L2-L01",
              status: "blocked",
              credited: false,
              providerEndpointKind: null,
              blockers: ["stage2_live_provider_p0_evidence_missing"],
              mainChatInvoked: false,
              modelInvoked: false,
              runIdPresent: false,
              taskSessionIdPresent: false,
              responsePreviewPresent: false,
            },
            {
              scenarioId: "L2-L05",
              status: "failed",
              credited: false,
              providerEndpointKind: "external_provider",
              blockers: ["live_provider_model_ranked_selection_trace_missing"],
              mainChatInvoked: true,
              modelInvoked: true,
              runIdPresent: true,
              taskSessionIdPresent: true,
              responsePreviewPresent: true,
            },
          ],
        },
        controlPlane: {
          ready: true,
          requiredCount: 10,
          attemptedCount: 10,
          passedCount: 10,
          failedIds: [],
          coverage: [
            {
              id: "direct_answer",
              passed: true,
              evidence: ["AgentRun DirectAnswer trace"],
              blockers: [],
            },
          ],
          blockers: [],
        },
        memoryProposal: {
          ready: true,
          requiredCount: 8,
          attemptedCount: 8,
          passedCount: 8,
          failedIds: [],
          coverage: [
            {
              id: "M2-01",
              passed: true,
              evidence: ["MR-01 pending memory proposal"],
              blockers: [],
            },
          ],
          blockers: [],
        },
        failureRecovery: {
          ready: true,
          requiredCount: 10,
          attemptedCount: 10,
          passedCount: 10,
          failedIds: [],
          coverage: [
            {
              id: "R2-01",
              passed: true,
              evidence: [
                "missing_workspace_file_blocker",
                "blocked_missing_source_state",
                "user_next_action_or_terminal_explanation",
                "no_fake_file_read_completion",
              ],
              blockers: [],
            },
          ],
          blockers: [],
        },
        finalDelivery: {
          ready: true,
          p0ScenarioCount: 24,
          finalDeliveryEvidenceCount: 24,
          finalDoneOverclaimCount: 0,
          blockers: [],
        },
        safety: {
          silentDurableWriteCount: 0,
          hiddenLegacyFallbackCount: 0,
          fakeBrowserEvidenceCount: 0,
          fakeLiveEvidenceCount: 0,
          localProviderCreditedAsLiveCount: 0,
          unscopedPermissionReplayCount: 0,
          finalDoneOverclaimCount: 0,
        },
        artifacts: [
          {
            kind: "stage1_browser_dogfood",
            path: "frontend/test-results/main-chat-stage1-dogfood-report.json",
            digest:
              "bytes:25422 hash:sha256:b53415fe64b623298be32b93fe55d3c45b7941c65d94e1ce6f3c716db8ade678",
            status: "loaded",
          },
          {
            kind: "manual_dogfood",
            path: "frontend/test-results/main-chat-stage2-manual-dogfood-report.json",
            digest: null,
            status: "missing",
          },
          {
            kind: "live_provider",
            path: "frontend/test-results/main-chat-stage2-live-provider-report.json",
            digest: null,
            status: "not_loaded",
          },
        ],
      } as T);
    case "validate_main_chat_agent_stage2_manual_dogfood_artifact":
      return Promise.resolve({
        attempted: false,
        ready: false,
        reviewerCount: 0,
        requiredScenarioCount: 24,
        attemptedScenarioCount: 0,
        passedScenarioCount: 0,
        missingScenarioIds: ["S2-D01"],
        failedScenarioIds: ["S2-D01"],
        traceIdsPresent: false,
        artifactDigest: null,
        blockers: ["stage2_manual_dogfood_evidence_missing"],
      } as T);
    case "list_mcp_servers":
      return Promise.resolve([
        {
          name: "filesystem",
          command: "npx",
          args: ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
          tool_count: 2,
        },
      ] as T);
    case "list_mcp_audit_logs":
      return Promise.resolve([
        {
          id: 1,
          tool_name: "write_file",
          arguments: '{"path":"/tmp/demo.txt","content":"hello"}',
          result: "工具执行成功",
          success: true,
          pii_found: true,
          created_at: new Date(Date.now() - 60000).toISOString(),
        },
      ] as T);
    case "clear_mcp_audit_logs":
      return Promise.resolve(3 as T);
    case "list_mcp_tools":
      return Promise.resolve([
        { name: "read_file", description: "读取文件内容" },
        { name: "write_file", description: "写入文件内容" },
      ] as T);
    case "list_mcp_templates":
      return Promise.resolve([
        {
          id: "filesystem",
          name: "本地文件系统",
          description: "读取和写入本地文件",
          command: "npx",
          args: ["-y", "@modelcontextprotocol/server-filesystem", "{{rootPath}}"],
          required_args: ["rootPath"],
          arg_labels: { rootPath: "允许访问的根目录路径" },
          tags: ["file", "filesystem", "local"],
        },
      ] as T);
    case "list_tool_manifests":
      return Promise.resolve([
        {
          id: "file.read",
          name: "file.read",
          description: "读取文件",
          parameters: {},
          permission_level: "low",
          risk_level: "low",
          version: "1.0.0",
          source: { type: "BuiltIn" },
          capabilities: ["read", "filesystem"],
          requires_confirmation: false,
          enabled: true,
          declarative_only: false,
          action_type: "read",
          tags: ["execution"],
        },
        {
          id: "web.search",
          name: "web.search",
          description: "搜索网页",
          parameters: {},
          permission_level: "low",
          risk_level: "low",
          version: "1.0.0",
          source: { type: "BuiltIn" },
          capabilities: ["network"],
          requires_confirmation: false,
          enabled: true,
          declarative_only: false,
          action_type: "read",
          tags: ["execution", "web"],
        },
      ] as T);
    case "run_multi_strategy_agent_preview":
      return Promise.resolve({
        runId: "run-preview-1",
        strategyKind: "react",
        payloadKind: "react",
        userOutput: "Preview response",
        proposalIds: [],
        warnings: [],
        metadataSafeSummary: {
          selectedStrategyKind: "react",
          taskKind: "conversation",
          riskLevel: "low",
          hasPolicyContext: true,
          governanceDecisionKind: "allow",
          reasonCode: "default_react",
        },
        governanceDecisionKind: "allow",
      } as T);
    case "check_runtime_migration_gate":
      return Promise.resolve({
        defaultChatUnchanged: true,
        previewPathHealthy: true,
        metadataSafeTraceReady: true,
        fallbackAvailable: true,
        noExternalWrites: true,
        proposalFirstPreserved: true,
        blockingReasons: [],
      } as T);
    case "check_controlled_chat_pilot_eligibility":
      return Promise.resolve({
        eligible: true,
        requiredCleanRuns: 3,
        cleanRunCount: 3,
        checkedRunIds: ["run-preview-clean-3", "run-preview-clean-2", "run-preview-clean-1"],
        blockingReasons: [],
        lastGateReport: {
          defaultChatUnchanged: true,
          previewPathHealthy: true,
          metadataSafeTraceReady: true,
          fallbackAvailable: true,
          noExternalWrites: true,
          proposalFirstPreserved: true,
          blockingReasons: [],
        },
        defaultChatUnchanged: true,
      } as T);
    case "record_controlled_pilot_promotion_evidence":
      return Promise.resolve({
        evidenceId: "ev_promotion_1",
        created: true,
        pilotRunId: _args?.input?.pilotRunId ?? "run-controlled-pilot-1",
        promotedAt: _args?.input?.promotedAt ?? new Date().toISOString(),
      } as T);
    case "get_controlled_pilot_promotion_evidence_summary":
      return Promise.resolve({
        promotedCount: 2,
        recentPromotedPilotRunIds: ["run-controlled-pilot-2", "run-controlled-pilot-1"],
        latestPromotionTimestamp: "2026-05-30T01:02:03Z",
        sourceTargetMismatchBlockCount: 1,
      } as T);
    case "check_controlled_pilot_promotion_readiness":
      return Promise.resolve({
        ready: true,
        requiredPromotions: 3,
        promotedCount: 3,
        recentPromotedPilotRunIds: [
          "run-controlled-pilot-3",
          "run-controlled-pilot-2",
          "run-controlled-pilot-1",
        ],
        latestPromotionTimestamp: "2026-05-30T03:04:05Z",
        sourceTargetMismatchBlockCount: 0,
        metadataSafeEvidenceReady: true,
        defaultChatUnchanged: true,
        blockingReasons: [],
      } as T);
    case "draft_controlled_chat_migration_plan":
      return Promise.resolve({
        draftReady: true,
        readinessReport: {
          ready: true,
          requiredPromotions: 3,
          promotedCount: 3,
          recentPromotedPilotRunIds: [
            "run-controlled-pilot-3",
            "run-controlled-pilot-2",
            "run-controlled-pilot-1",
          ],
          latestPromotionTimestamp: "2026-05-30T03:04:05Z",
          sourceTargetMismatchBlockCount: 0,
          metadataSafeEvidenceReady: true,
          defaultChatUnchanged: true,
          blockingReasons: [],
        },
        migrationScope: [
          "Draft scope is limited to a human-reviewed controlled pilot discussion; default Chat remains unchanged.",
        ],
        requiredPreconditions: [
          "Separate human approval is required before any migration implementation work begins.",
        ],
        rollbackPlan: ["Disable the controlled pilot entry and keep default Chat unchanged."],
        fallbackPlan: ["Use the existing default Chat send path whenever the pilot is blocked."],
        testPlan: ["Verify send_message and start_stream_message do not call this draft command."],
        manualReviewRequired: true,
        notAutomaticMigration: true,
        blockingReasons: [],
      } as T);
    case "record_controlled_chat_migration_review_decision":
      return Promise.resolve({
        recorded: true,
        evidenceId: "ev_review_decision_1",
        decisionKind: _args?.input?.decisionKind ?? "approve",
        draftReady: true,
        draftHash: "sha256:mock-migration-draft",
        createdAt: "2026-05-31T01:02:03Z",
        blockingReasons: [],
      } as T);
    case "get_controlled_chat_migration_review_decision_summary":
      return Promise.resolve({
        latestDecision: {
          evidenceId: "ev_review_decision_1",
          decisionKind: "request_rework",
          draftReady: true,
          draftHash: "sha256:mock-migration-draft",
          createdAt: "2026-05-31T01:02:03Z",
        },
        approvedCount: 1,
        reworkRejectCount: 2,
        latestTimestamp: "2026-05-31T01:02:03Z",
        blockingReasons: [],
      } as T);
    case "check_controlled_chat_migration_implementation_gate":
      return Promise.resolve({
        implementationEligible: true,
        latestDecision: {
          evidenceId: "ev_review_decision_2",
          decisionKind: "approve",
          draftReady: true,
          draftHash: "sha256:mock-migration-draft",
          createdAt: "2026-05-31T02:03:04Z",
        },
        readinessReport: {
          ready: true,
          requiredPromotions: 3,
          promotedCount: 3,
          recentPromotedPilotRunIds: [
            "run-controlled-pilot-3",
            "run-controlled-pilot-2",
            "run-controlled-pilot-1",
          ],
          latestPromotionTimestamp: "2026-05-30T03:04:05Z",
          sourceTargetMismatchBlockCount: 0,
          metadataSafeEvidenceReady: true,
          defaultChatUnchanged: true,
          blockingReasons: [],
        },
        draftHashMatched: true,
        approvedAfterLatestDraft: true,
        blockingReasons: [],
      } as T);
    case "run_controlled_chat_migration_shadow_run":
      return Promise.resolve({
        shadowRunReady: true,
        shadowRunId: "run-shadow-1",
        implementationGateReport: {
          implementationEligible: true,
          latestDecision: {
            evidenceId: "ev_review_decision_2",
            decisionKind: "approve",
            draftReady: true,
            draftHash: "sha256:mock-migration-draft",
            createdAt: "2026-05-31T02:03:04Z",
          },
          readinessReport: {
            ready: true,
            requiredPromotions: 3,
            promotedCount: 3,
            recentPromotedPilotRunIds: [
              "run-controlled-pilot-3",
              "run-controlled-pilot-2",
              "run-controlled-pilot-1",
            ],
            latestPromotionTimestamp: "2026-05-30T03:04:05Z",
            sourceTargetMismatchBlockCount: 0,
            metadataSafeEvidenceReady: true,
            defaultChatUnchanged: true,
            blockingReasons: [],
          },
          draftHashMatched: true,
          approvedAfterLatestDraft: true,
          blockingReasons: [],
        },
        strategyKind: "react",
        payloadKind: "react",
        metadataSafeSummary: {
          descriptorKind: _args?.input?.boundedTestPromptDescriptor ?? "default_readiness_probe",
          allowWrites: false,
          metadataSafe: true,
          reasonCode: "default_react",
          riskLevel: "low",
        },
        warnings: ["shadow runtime forced allowWrites=false"],
        blockingReasons: [],
      } as T);
    case "record_controlled_chat_migration_shadow_review_decision":
      return Promise.resolve({
        recorded: true,
        evidenceId: "ev_shadow_review_1",
        shadowRunId: _args?.input?.shadowRunId ?? "run-shadow-1",
        decisionKind: _args?.input?.decisionKind ?? "approve",
        readinessSummaryDigest: "sha256:mock-shadow-readiness",
        createdAt: "2026-05-31T04:05:06Z",
        blockingReasons: [],
      } as T);
    case "get_controlled_chat_migration_shadow_review_summary":
      return Promise.resolve({
        latestDecision: {
          evidenceId: "ev_shadow_review_1",
          shadowRunId: "run-shadow-1",
          decisionKind: "approve",
          reviewerNoteChecksum: "sha256:reviewer-note",
          reviewerNoteLength: 0,
          reviewerNoteCategory: "none",
          readinessSummaryDigest: "sha256:mock-shadow-readiness",
          createdAt: "2026-05-31T04:05:06Z",
        },
        approvedCount: 1,
        reworkRejectCount: 0,
        latestTimestamp: "2026-05-31T04:05:06Z",
        blockingReasons: [],
      } as T);
    case "check_controlled_chat_cutover_readiness":
      return Promise.resolve({
        cutoverPlanningEligible: true,
        implementationGateReport: {
          implementationEligible: true,
          latestDecision: {
            evidenceId: "ev_review_decision_2",
            decisionKind: "approve",
            draftReady: true,
            draftHash: "sha256:mock-migration-draft",
            createdAt: "2026-05-31T02:03:04Z",
          },
          readinessReport: {
            ready: true,
            requiredPromotions: 3,
            promotedCount: 3,
            recentPromotedPilotRunIds: [
              "run-controlled-pilot-3",
              "run-controlled-pilot-2",
              "run-controlled-pilot-1",
            ],
            latestPromotionTimestamp: "2026-05-30T03:04:05Z",
            sourceTargetMismatchBlockCount: 0,
            metadataSafeEvidenceReady: true,
            defaultChatUnchanged: true,
            blockingReasons: [],
          },
          draftHashMatched: true,
          approvedAfterLatestDraft: true,
          blockingReasons: [],
        },
        latestShadowReviewDecision: {
          evidenceId: "ev_shadow_review_1",
          shadowRunId: "run-shadow-1",
          decisionKind: "approve",
          reviewerNoteChecksum: "sha256:reviewer-note",
          reviewerNoteLength: 0,
          reviewerNoteCategory: "none",
          readinessSummaryDigest: "sha256:mock-shadow-readiness",
          createdAt: "2026-05-31T04:05:06Z",
        },
        verifiedShadowRunId: "run-shadow-1",
        readinessSummaryDigest: "sha256:mock-shadow-readiness",
        defaultChatUnchanged: true,
        requiredEvidenceReady: true,
        blockingReasons: [],
        metadataSafeSummary: {
          cutoverReadinessGate: "controlled_chat_cutover_planning",
          metadataSafe: true,
          planningOnly: true,
          implementationEligible: true,
          shadowRunReady: true,
          latestShadowReviewDecisionKind: "approve",
          contentStorage: "none",
          toolStorage: "none",
        },
      } as T);
    case "run_controlled_chat_cutover_candidate":
      return Promise.resolve({
        candidateReady: true,
        candidateRunId: "run-candidate-1",
        outputPreview: "Cutover candidate: react / react",
        userOutput: "Candidate-only answer",
        contractShape: "send_message_compatible",
        metadataSafeSummary: {
          candidateAdapter: "controlled_chat_cutover_candidate",
          metadataSafe: true,
          nonDefault: true,
          allowWrites: false,
          maxToolCalls: 0,
          chatHistoryStorage: "none",
          proposalStorage: "none",
          memoryStorage: "none",
        },
        warnings: ["candidate runtime forced allowWrites=false"],
        blockingReasons: [],
      } as T);
    case "record_controlled_chat_cutover_candidate_review_decision":
      return Promise.resolve({
        recorded: true,
        evidenceId: "ev_candidate_review_1",
        candidateRunId: _args?.input?.candidateRunId ?? "run-candidate-1",
        decisionKind: _args?.input?.decisionKind ?? "approve",
        contractShape: "send_message_compatible",
        candidateSummaryDigest: "sha256:mock-candidate-summary",
        createdAt: "2026-05-31T06:07:08Z",
        blockingReasons: [],
      } as T);
    case "get_controlled_chat_cutover_candidate_review_summary":
      return Promise.resolve({
        latestDecision: {
          evidenceId: "ev_candidate_review_1",
          candidateRunId: "run-candidate-1",
          decisionKind: "approve",
          contractShape: "send_message_compatible",
          candidateSummaryDigest: "sha256:mock-candidate-summary",
          reviewerNoteChecksum: null,
          reviewerNoteLength: 0,
          reviewerNoteCategory: "none",
          createdAt: "2026-05-31T06:07:08Z",
        },
        approvedCount: 1,
        reworkRejectCount: 0,
        latestTimestamp: "2026-05-31T06:07:08Z",
        blockingReasons: [],
      } as T);
    case "check_controlled_chat_cutover_candidate_promotion_readiness":
      return Promise.resolve({
        ready: true,
        cutoverReadinessEligible: true,
        requiredApprovedCandidates: _args?.input?.requiredApprovedCandidates ?? 1,
        approvedCandidateCount: 1,
        latestDecision: {
          evidenceId: "ev_candidate_review_1",
          candidateRunId: "run-candidate-1",
          decisionKind: "approve",
          contractShape: "send_message_compatible",
          candidateSummaryDigest: "sha256:mock-candidate-summary",
          reviewerNoteChecksum: null,
          reviewerNoteLength: 0,
          reviewerNoteCategory: "none",
          createdAt: "2026-05-31T06:07:08Z",
        },
        approvedCandidates: [
          {
            evidenceId: "ev_candidate_review_1",
            candidateRunId: "run-candidate-1",
            contractShape: "send_message_compatible",
            candidateSummaryDigest: "sha256:mock-candidate-summary",
            runReadinessDigest: "sha256:mock-candidate-run-readiness",
            decisionCreatedAt: "2026-05-31T06:07:08Z",
            ready: true,
            blockingReasons: [],
          },
        ],
        defaultChatUnchanged: true,
        blockingReasons: [],
        metadataSafeSummary: {
          promotionReadinessGate: "controlled_chat_cutover_candidate",
          metadataSafe: true,
          readOnly: true,
          notAutomaticMigration: true,
          defaultChatUnchanged: true,
          approvedCandidateCount: 1,
        },
        checkedAt: "2026-05-31T06:08:00Z",
      } as T);
    case "get_agent_run":
      if (_args?.runId === "run-preview-1") {
        return Promise.resolve(mockPreviewAgentRun as T);
      }
      return Promise.resolve(null as T);
    case "list_agent_runs":
      return Promise.resolve([] as T);
    case "list_agent_runs_for_session":
      return Promise.resolve([] as T);
    case "get_last_model_error":
      return Promise.resolve(null as T);
    case "get_pending_proposals":
      return Promise.resolve([] as T);
    case "list_proposals":
      return Promise.resolve([] as T);
    case "get_review_center_view_model":
      return Promise.resolve({
        data: {
          items: [],
          summary: {
            total: 0,
            actionRequiredCount: 0,
            blockedActionCount: 0,
            byStatus: {},
            byRisk: {},
            byMaterializationStatus: {},
          },
        },
        status: "empty",
        lastUpdatedAt: new Date().toISOString(),
        source: "backend-readmodel",
        evidenceRefs: [],
        warnings: [],
        actions: { primary: [] },
      } as T);
    case "batch_accept_low_risk_proposals":
      return Promise.resolve(0 as T);
    case "accept_proposal":
      return Promise.resolve({
        success: true,
        patchResult: {
          patchId: _args?.proposalId ?? _args?.proposal_id ?? "proposal-1",
          success: true,
          path: "mock",
          operation: "accept",
        },
        effectStatus: "confirmed",
        proposalProjectionStatus: "confirmed",
        warnings: [],
      } as T);
    case "reject_proposal":
    case "edit_proposal":
    case "postpone_proposal":
      return Promise.resolve(undefined as T);
    case "draft_edit_memory_proposal":
      return Promise.resolve({
        proposalId: _args?.proposalId ?? _args?.proposal_id ?? "proposal-memory-1",
        draftOnly: true,
        durableWriteExecuted: false,
        originalProvenancePreserved: true,
        status: "pending",
        beforeDigest: "before",
        afterDigest: "after",
      } as T);
    case "list_memory_assets":
      return Promise.resolve([
        {
          memoryId: "memory:active-1",
          proposalId: "proposal-memory-1",
          content: "Prefer concise reviews.",
          scope: "workspace",
          category: "preference",
          riskLevel: "low",
          status: "materialized",
          materializationStatus: "materialized",
          createdBy: "assistant",
          acceptedBy: "user",
          materializedViewId: "memory_view:1",
          materializedViewVersion: 1,
          evidenceIds: ["proposal-memory-1"],
          confidence: 0.84,
          conflictIds: [],
        },
      ] as T);
    case "rollback_memory_asset":
      return Promise.resolve({
        record: {
          memoryId: _args?.memoryId ?? _args?.memory_id ?? "memory:active-1",
          proposalId: "proposal-memory-1",
          content: "Prefer concise reviews.",
          scope: "workspace",
          category: "preference",
          riskLevel: "low",
          status: "rolled_back",
          materializationStatus: "not_required",
          createdBy: "assistant",
          materializedViewVersion: 2,
          evidenceIds: ["proposal-memory-1"],
          confidence: 0.84,
          conflictIds: [],
          rolledBackByEventId: "memory_rollback:1",
        },
        rollbackEvent: {
          rollbackEventId: "memory_rollback:1",
          memoryId: _args?.memoryId ?? _args?.memory_id ?? "memory:active-1",
          proposalId: "proposal-memory-1",
          requestedBy: "user",
          reason: _args?.reason ?? "mock rollback",
          previousStatus: "materialized",
          nextStatus: "rolled_back",
          affectedMaterializedViewIds: ["memory_view:1"],
          affectedRuntimeSurfaceIds: ["main_chat_context"],
          createdAt: new Date().toISOString(),
          auditDigest: "sha256:mock",
        },
        materializedView: {
          materializedViewId: "memory_view:1",
          version: 2,
          activeMemoryIds: [],
          runtimeSurfaceIds: ["main_chat_context"],
          updatedAt: new Date().toISOString(),
          contentDigest: "digest",
        },
      } as T);
    case "draft_memory_correction_proposal":
      return Promise.resolve({
        proposalId: "proposal:memory-correction:mock",
        memoryId: _args?.memoryId ?? _args?.memory_id ?? "memory:active-1",
        action: "correct",
        status: "review_required",
      } as T);
    case "draft_memory_archive_proposal":
      return Promise.resolve({
        proposalId: "proposal:memory-archive:mock",
        memoryId: _args?.memoryId ?? _args?.memory_id ?? "memory:active-1",
        action: "archive",
        status: "review_required",
      } as T);
    case "draft_memory_stop_recall_proposal":
      return Promise.resolve({
        proposalId: "proposal:memory-stop-recall:mock",
        memoryId: _args?.memoryId ?? _args?.memory_id ?? "memory:active-1",
        action: "stop_recall",
        status: "review_required",
      } as T);
    case "privacy_erase_memory_asset":
      return Promise.resolve({
        memoryId: _args?.memoryId ?? _args?.memory_id ?? "memory:active-1",
        erasedAt: new Date().toISOString(),
        materializedView: {
          materializedViewId: "memory_view:global",
          version: 2,
          activeMemoryIds: [],
          runtimeSurfaceIds: ["runtime:global"],
          updatedAt: new Date().toISOString(),
          contentDigest: "erased-view",
        },
        canonicalMutation: {
          eventId: "memory-erase:mock",
          aggregateKind: "memory_lifecycle",
          aggregateId: _args?.memoryId ?? _args?.memory_id ?? "memory:active-1",
          mutationKind: "deleted",
          payloadDigest: "erased",
          createdAt: new Date().toISOString(),
        },
        canonicalCommitted: true,
        projectionState: "applied",
      } as T);
    case "list_stage4_knowledge_asset_inventory":
      return Promise.resolve({
        inventoryId: "stage4_knowledge_inventory:mock",
        root: "/mock",
        loadedAssets: [
          {
            assetId: "knowledge:USER.md",
            relativePath: "USER.md",
            source: "/mock:USER.md",
            digest: "1234567890abcdef",
            sizeBytes: 42,
            truncated: false,
            reason: "bounded user profile context surface",
            contextOnly: true,
          },
          {
            assetId: "knowledge:MEMORY.md",
            relativePath: "MEMORY.md",
            source: "/mock:MEMORY.md",
            digest: "abcdef1234567890",
            sizeBytes: 64,
            truncated: false,
            reason: "bounded curated memory context surface",
            contextOnly: true,
          },
        ],
        skippedAssets: [
          {
            assetId: "knowledge:SOUL.md",
            relativePath: "SOUL.md",
            source: "/mock:SOUL.md",
            reason: "missing",
          },
          {
            assetId: "knowledge:skills/other/SKILL.md",
            relativePath: "skills/other/SKILL.md",
            source: "/mock:skills/other/SKILL.md",
            reason: "unselected_skill",
            selectedSkillId: "other",
          },
        ],
      } as T);
    case "list_tool_permissions":
      return Promise.resolve([
        {
          id: "perm-1",
          toolName: "builtin_echo",
          source: "builtin",
          riskLevel: "low",
          actionType: "mcp_tool_call",
          policy: "allow_until_revoked",
          createdAt: new Date().toISOString(),
        },
      ] as T);
    case "revoke_tool_permission":
      return Promise.resolve(true as T);
    case "list_plugins":
    case "reload_plugins":
      return Promise.resolve([
        {
          manifest: {
            id: "local-demo",
            name: "Local Demo",
            version: "0.1.0",
            description: "本地插件示例",
            author: "OpenLife",
            tools: [],
            skills: [],
            permissions: ["read"],
            enabled: false,
            trustLevel: "local",
          },
          path: "/tmp/openlife/plugins/local-demo/plugin.json",
          enabled: false,
        },
      ] as T);
    case "enable_plugin":
    case "disable_plugin":
      return Promise.resolve(undefined as T);
    case "count_memory_chunks":
      return Promise.resolve(42 as T);
    case "rebuild_memory_index":
      return Promise.resolve({
        processed: 12,
        indexed: 10,
        skipped: 2,
      } as T);
    case "check_ollama_status":
      return Promise.resolve(true as T);
    case "get_policy_router_status":
      return Promise.resolve({
        activeAuthority: "IntentFrame + PolicyRouter",
        authorityChain: [
          "user_input",
          "IntentFrame",
          "PolicyRouter",
          "AgentIngressDecision",
          "OpenLifeTurnRuntime",
          "MainChatKernel",
        ],
        routeOutputs: [
          "direct_answer",
          "read_only_tool",
          "proposal_only_write",
          "plan_draft",
          "ask_clarification",
          "governed_blocker",
          "confirmation_request",
        ],
        appStateOldRoutersPresent: false,
        diagnosticsSurface: "policy_router_status",
      } as T);
    case "get_model_router_status":
      return Promise.resolve({
        enabled: false,
        providers: [
          { name: "ollama", enabled: true, available: false, healthIsEstimated: true },
          { name: "openai", enabled: true, available: true, healthIsEstimated: true },
        ],
        lastCheckAt: new Date().toISOString(),
      } as T);
    case "get_system_diagnostics":
      return Promise.resolve({
        policy_router: {
          activeAuthority: "IntentFrame + PolicyRouter",
          authorityChain: [
            "user_input",
            "IntentFrame",
            "PolicyRouter",
            "AgentIngressDecision",
            "OpenLifeTurnRuntime",
            "MainChatKernel",
          ],
          routeOutputs: [
            "direct_answer",
            "read_only_tool",
            "proposal_only_write",
            "plan_draft",
            "ask_clarification",
            "governed_blocker",
            "confirmation_request",
          ],
          appStateOldRoutersPresent: false,
          diagnosticsSurface: "policy_router_status",
        },
        mcp_server_count: 1,
        mcp_tool_count: 2,
        mcp_recent_audit_count: 1,
        mcp_recent_pii_count: 1,
        memory_chunk_count: 42,
        vector_corrupt_embedding_count: 0,
        ollama_online: true,
        local_model: "llama3",
        resolved_local_model: "llama3:latest",
        prefer_local_model: true,
        cloud_api_configured: false,
        cloud_provider: "DeepSeek",
        cloud_api_validated: false,
        cloud_api_last_error: null,
        chat_ready: true,
        readiness_issues: [],
        data_dir: "/tmp/openlife-test",
        active_data_dir: "/tmp/openlife-test",
        database_status: "ok",
        startup_warnings: [],
        life_model_ready: true,
        app_version: "0.1.0",
        model_empty: false,
        chat_session_count: 3,
        usage_ready: true,
        usage_readiness_issues: [],
        data_files: {
          messages_db_exists: true,
          messages_db_size_mb: 1.2,
          vectors_db_exists: true,
          vectors_db_size_mb: 0.8,
          mcp_audit_db_exists: true,
          mcp_audit_db_size_mb: 0.1,
          config_yaml_exists: true,
          life_model_yaml_exists: true,
        },
        ollama_models: [
          { name: "llama3", size_mb: 4500 },
          { name: "qwen2.5", size_mb: 3200 },
        ],
        config_source: "env+default",
      } as T);
    case "get_life_state_projection":
      return Promise.resolve({
        version: "life_state_projection_v1",
        generatedAt: new Date().toISOString(),
        pending: {
          pendingProposalCount: 0,
          editedProposalCount: 0,
          totalReviewRequiredCount: 0,
          highRiskReviewRequiredCount: 0,
          proposalStoreStatus: "ok",
          requiresUserAction: false,
        },
        readiness: {
          chatReady: true,
          usageReady: true,
          lifeModelReady: true,
          modelEmpty: false,
          databaseStatus: "ok",
          readinessIssues: [],
          usageReadinessIssues: [],
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
        safeMode: {
          active: false,
          reason: "系统当前未处于 Safe Mode。",
          sourceRefs: [],
        },
        toolPermissions: {
          totalCount: 0,
          activeCount: 0,
          consumedCount: 0,
          allowCount: 0,
          denyCount: 0,
          askEveryTimeCount: 0,
          allowOnceCount: 0,
          allowUntilRevokedCount: 0,
        },
        safePaths: [],
        surfaces: ["today", "mailbox", "chat", "companion", "life_model", "settings"].map(
          surface => ({
            surface,
            pendingReviewCount: 0,
            editedReviewCount: 0,
            totalReviewRequiredCount: 0,
            readinessStatus: "ready",
            taskStatus: "idle",
            safeModeActive: false,
            waitingPermissionCount: 0,
            activeToolPermissionCount: 0,
          })
        ),
        sourceRefs: [
          "diagnostics",
          "proposal_store:pending_and_edited",
          "main_chat_agent_session_store",
          "tool_permission_store",
          "config:safe_paths",
        ],
      } as T);
    case "save_chat_message":
      return Promise.resolve(undefined as T);
    case "register_mcp_server":
    case "unregister_mcp_server":
      return Promise.resolve(undefined as T);
    case "execute_tool_call":
      return Promise.resolve({
        toolRef: { id: "unknown_tool", source: "unknown" },
        actionRef: "unknown_action",
        status: "success",
        requiresConfirmation: false,
        privacyWarningCount: 0,
      } as T);
    case "inspect_mcp_call":
      return Promise.resolve({
        permission_level: "medium",
        pii_found: true,
        findings: [{ path: "$.query", privacy_type: "Email", matched: "test@example.com" }],
        sanitized_arguments: { query: "帮我搜索 <EMAIL_0> 的公开信息" },
        requires_confirmation: true,
      } as T);
    case "search_memory":
      return Promise.resolve({
        hits: [],
        embeddingProfile: {
          id: "embedding:hash:test:dim:384",
          route: "deterministic_hash",
          provider: "openlife",
          model: "openlife-hash-ngram-v1",
          dimension: 384,
        },
        embeddingReceipt: {
          requestId: "embedding-request-test",
          route: "deterministic_hash",
          profileId: "embedding:hash:test:dim:384",
          status: "not_attempted",
          source: "deterministic_hash",
          routeReasonCode: "configured_deterministic_hash",
          cacheHit: false,
        },
        vectorStatus: "ready",
        routeQuality: "deterministic_hash_approximation",
      } as T);
    // Milestone D mocks
    case "get_hot_cache":
      return Promise.resolve({
        identity_summary: "你是测试用户，成为更好的自己。你的核心哲学是：活在当下。",
        top_values: ["健康 (保持身体健康)", "学习 (持续学习成长)"],
        current_goals: ["完成项目 (优先级: 1, 进度: 50%)", "○ 每日目标: 早起"],
        recent_state: "心情: happy，当前专注: 工作",
        last_refreshed: new Date().toISOString(),
        life_model_version: "",
      } as T);
    case "archive_low_access_memories":
      return Promise.resolve([] as T);
    case "restore_archived_chunks":
      return Promise.resolve({
        owner: _args?.owner,
        disposition: "active",
        changed: true,
        canonicalCommitted: true,
        revision: 2,
        outboxEventId: "memory-retrieval-2",
        projectionState: "applied",
      } as T);
    case "list_archived_chunks":
      return Promise.resolve([] as T);
    case "get_memory_tier_stats":
      return Promise.resolve({ total: 0, tier1: 0, tier2: 0, tier3: 0, archived: 0 } as T);
    case "get_danger_action_preflight": {
      const actionType = String(_args?.actionType ?? _args?.action_type ?? "data_export");
      const safeMode = Boolean(_args?.safeMode ?? _args?.safe_mode);
      const targetIds = (_args?.targetIds ?? _args?.target_ids ?? []) as string[];
      const affectedCount = Number(
        _args?.affectedCount ?? _args?.affected_count ?? targetIds.length ?? 0
      );
      const mutating = [
        "data_import_overwrite",
        "data_import_abandon_recovery",
        "mcp_audit_cleanup",
        "mcp_audit_key_rotation",
        "agent_run_delete",
        "agent_run_bulk_delete",
        "vector_rebuild",
      ].includes(actionType);
      const confirmationPhrases: Record<string, string> = {
        data_import_overwrite: "IMPORT",
        data_import_abandon_recovery: "PRESERVE CURRENT",
        mcp_audit_cleanup: "CLEANUP",
        mcp_audit_key_rotation: "ROTATE",
        agent_run_delete: "DELETE RUN",
        agent_run_bulk_delete: "DELETE RUNS",
        vector_rebuild: "REBUILD",
      };
      const digest = `bytes:${actionType.length + targetIds.join("|").length + affectedCount} hash:sha256:${"a".repeat(64)}`;
      const labels: Record<string, string> = {
        data_export: "导出本地 LifeModel、聊天记录和向量记忆到本地 JSON 文件。",
        data_import_overwrite: "覆盖当前 LifeModel、聊天记录和向量记忆。",
        data_import_abandon_recovery:
          "保留当前 canonical 数据，并以 abandoned_preserving_current 终止中断的导入。",
        mcp_audit_export:
          "导出最近 MCP 审计日志元数据，可能包含工具输入参数文本和工具执行结果文本。",
        mcp_audit_cleanup: "删除超过保留期限的本地 MCP 审计日志。",
        mcp_audit_key_rotation: "轮换本地 MCP 审计加密 epoch。",
        agent_run_delete: "删除选中的 AgentRun 运行记录；不展开 transcript。",
        agent_run_bulk_delete: "批量删除选中的 AgentRun 运行记录；不展开 transcript。",
        vector_rebuild: "基于现有聊天消息重建本地向量索引；不展示原始消息。",
      };
      const finalCommands: Record<string, string> = {
        data_export: "export_all_data",
        data_import_overwrite: "import_all_data",
        data_import_abandon_recovery: "abandon_governed_data_import_recovery",
        mcp_audit_export: "export_mcp_audit_logs",
        mcp_audit_cleanup: "cleanup_mcp_audit_logs",
        mcp_audit_key_rotation: "rotate_mcp_audit_key",
        agent_run_delete: "delete_agent_run",
        agent_run_bulk_delete: "delete_agent_run",
        vector_rebuild: "rebuild_memory_index",
      };
      return Promise.resolve({
        actionType,
        riskTier:
          actionType === "data_import_overwrite" ||
          actionType === "data_import_abandon_recovery" ||
          actionType === "mcp_audit_key_rotation"
            ? "critical"
            : "high",
        scopeSummary: labels[actionType] ?? "未知危险动作。",
        dataCategories: actionType.startsWith("mcp_audit")
          ? actionType === "mcp_audit_export"
            ? ["mcp_audit_metadata", "tool_metadata", "tool_input_text", "tool_output_text"]
            : ["mcp_audit_metadata", "tool_metadata"]
          : actionType === "data_import_abandon_recovery"
            ? [
                "governed_import_journal_metadata",
                "canonical_owner_digest_evidence",
                "state_projection_delivery_metadata",
              ]
            : actionType.startsWith("agent_run")
              ? ["agent_run_metadata", "run_trace_metadata"]
              : actionType === "vector_rebuild"
                ? ["messages_metadata", "vectors"]
                : ["life_model", "messages", "vectors"],
        writesDurableState: mutating,
        privacySensitive: true,
        externalTransmission: "not_sent_externally",
        dryRunAvailable: false,
        backupStatus:
          actionType === "data_import_overwrite"
            ? "lifemodel_snapshot_only_other_owners_forward_recovery"
            : actionType === "data_import_abandon_recovery"
              ? "not_applicable_preserves_current_canonical_data"
              : actionType === "vector_rebuild"
                ? "rollback_previous_vectors_on_failure"
                : mutating
                  ? "none"
                  : "not_required_read_only",
        requiresTypedConfirmation: Boolean(confirmationPhrases[actionType]),
        confirmationRequired: Boolean(confirmationPhrases[actionType]),
        confirmationPhrase: confirmationPhrases[actionType] ?? null,
        confirmationScopeDigest: digest,
        preflightId: `danger-preflight:sha256:${"b".repeat(64)}`,
        affectedItemCount: affectedCount,
        affectedItemDigest: digest,
        finalActionEnabled: !(safeMode && mutating),
        safeModeBlocked: safeMode && mutating,
        blockingReasons: safeMode && mutating ? ["safe_mode_blocks_durable_write"] : [],
        sourceRefs: [
          "settings_command:get_danger_action_preflight",
          `final_command:${finalCommands[actionType] ?? "unknown"}`,
          actionType.startsWith("agent_run") || actionType === "vector_rebuild"
            ? "governance:slice5c_danger_zone_consolidation"
            : "governance:slice5b_danger_action_preflight",
          `scope_digest:${digest}`,
        ],
      } as T);
    }
    case "abandon_governed_data_import_recovery":
      return Promise.resolve({
        success: true,
        status: "abandoned_preserving_current_restart_required",
        operation_id: _args?.operationId ?? _args?.operation_id,
        stage: "abandoned_preserving_current",
        recovery_terminalized: true,
        original_import_completed: false,
        rollback_completed: false,
        preserved_current_canonical_data: true,
        abandonment_mutated_canonical_owners: false,
        original_import_effect_state: "preserved_current_observed_per_owner",
        owner_resolution_counts: { before: 1, target: 2, other: 1 },
        resolution_evidence_count: 4,
        restart_required: true,
      } as T);
    case "get_governed_data_import_status":
      return Promise.resolve({
        status: "idle",
        operationId: null,
        stage: null,
        terminal: false,
        terminalAt: null,
        recoveryRequired: false,
        runtimeRecoveryIsolationActive: false,
        restartRequired: false,
        originalImportCompleted: false,
        rollbackCompleted: false,
        preservedCurrent: false,
        ownerCount: 0,
        resolutionEvidenceCount: 0,
        ownerResolutionCounts: { before: 0, target: 0, other: 0 },
        observedAt: new Date().toISOString(),
      } as T);
    case "export_mcp_audit_logs":
      return Promise.resolve({
        exported_at: new Date().toISOString(),
        entry_count: 0,
        days: _args?.days ?? 7,
        entries: [],
      } as T);
    case "cleanup_mcp_audit_logs":
      return Promise.resolve(0 as T);
    case "rotate_mcp_audit_key":
      return Promise.resolve(undefined as T);
    case "get_privacy_policy":
      return Promise.resolve({
        enabled: true,
        rules: [
          { ptype: "Phone", enabled: true, action: "Mask", custom_pattern: undefined },
          { ptype: "IdCard", enabled: true, action: "Block", custom_pattern: undefined },
          { ptype: "Email", enabled: true, action: "Mask", custom_pattern: undefined },
        ],
      } as T);
    case "export_all_data":
      return Promise.resolve({
        version: "2.0",
        app_version: "0.1.0",
        exported_at: new Date().toISOString(),
        life_model: {},
        messages: [],
        vectors: [],
      } as T);
    case "import_all_data":
      return Promise.resolve({
        success: true,
        legacy: false,
        governed_operation: true,
        warning: "metadata-safe",
        metadata_safe: true,
        durable_lifemodel_write: true,
        imported_message_count: args?.payload?.messages?.length ?? 0,
        imported_vector_count: args?.payload?.vectors?.length ?? 0,
      } as T);
    case "test_llm_connection":
      return Promise.resolve({
        ok: true,
        provider: _args?.config?.llm?.provider === "deepseek" ? "DeepSeek" : "OpenAI-compatible",
        message: "连接成功",
      } as T);
    case "set_privacy_policy":
      return Promise.resolve(undefined as T);
    default:
      return Promise.resolve({} as T);
  }
});
