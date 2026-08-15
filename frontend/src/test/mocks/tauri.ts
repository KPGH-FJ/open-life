import { vi } from "vitest";
import type { LifeModel, ChatMessage, StateHistoryEntry, StateAlert } from "@/types";
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
          relatedRunIds: ["run_mainchat_mock"],
          conversationId: "session-1",
          title: "mock goal",
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
        needsAttentionCount: 0,
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
      tasks: [],
      activeTask: {
        canonicalTaskId: "mainchat_task_mock",
        relatedRunIds: ["run_mainchat_mock"],
        conversationId: "conversation_mock",
        title: "mock goal",
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
        taskId: _args?.taskId ?? null,
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
    case "get_product_diagnostics_view_model":
      return Promise.resolve({
        generatedAt: new Date().toISOString(),
        status: "ready",
        appVersion: "0.1.0",
        runtimeBuild: {
          profile: "qa",
          gitSha: "mock-build",
          buildTime: "2026-08-14T00:00:00Z",
          currentExe: "/Applications/OpenLife.app/Contents/MacOS/openlife-tauri",
          binaryKind: "release_bundle",
          frontendMode: "bundled_dist",
          devUrl: "",
          frontendDist: "frontend/dist",
          dataDir: "/tmp/openlife-mock",
          devExtensionsEnabled: false,
          arbitraryMcpRegistrationEnabled: false,
          bundleIdentifier: "ai.openlife.desktop",
          productName: "OpenLife",
        },
        persistenceMode: "read_write",
        canonicalWritesAllowed: true,
        providerDispatchAllowed: true,
        toolDispatchAllowed: true,
        stores: [
          { store: "ConversationStore", status: "read_write_canonical", reasonCode: null },
          {
            store: "CanonicalTaskRuntimeStore",
            status: "read_write_canonical",
            reasonCode: null,
          },
        ],
        counts: {
          projectCount: 1,
          conversationCount: 1,
          taskCount: 0,
          activeTaskCount: 0,
          waitingTaskCount: 0,
          completedTaskCount: 0,
          failedTaskCount: 0,
          unresolvedAttentionCount: 0,
        },
        credentialBootstrap: { version: "v1", digest: "mock", purposes: [] },
        blockerCodes: [],
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
          "canonical_conversation_store",
          "tool_permission_store",
          "config:safe_paths",
        ],
      } as T);
    case "save_chat_message":
      return Promise.resolve(undefined as T);
    case "register_mcp_server":
    case "unregister_mcp_server":
      return Promise.resolve(undefined as T);
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
      const actionType = String(_args?.actionType ?? "data_export");
      const safeMode = Boolean(_args?.safeMode);
      const targetIds = (_args?.targetIds ?? []) as string[];
      const affectedCount = Number(_args?.affectedCount ?? targetIds.length ?? 0);
      const mutating = [
        "data_import_overwrite",
        "data_import_abandon_recovery",
        "mcp_audit_cleanup",
        "mcp_audit_key_rotation",
        "vector_rebuild",
      ].includes(actionType);
      const confirmationPhrases: Record<string, string> = {
        data_import_overwrite: "IMPORT",
        data_import_abandon_recovery: "PRESERVE CURRENT",
        mcp_audit_cleanup: "CLEANUP",
        mcp_audit_key_rotation: "ROTATE",
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
        vector_rebuild: "基于现有聊天消息重建本地向量索引；不展示原始消息。",
      };
      const finalCommands: Record<string, string> = {
        data_export: "export_all_data",
        data_import_overwrite: "import_all_data",
        data_import_abandon_recovery: "abandon_governed_data_import_recovery",
        mcp_audit_export: "export_mcp_audit_logs",
        mcp_audit_cleanup: "cleanup_mcp_audit_logs",
        mcp_audit_key_rotation: "rotate_mcp_audit_key",
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
          actionType === "vector_rebuild"
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
