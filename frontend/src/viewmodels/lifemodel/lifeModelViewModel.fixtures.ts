import type {
  AgentProposal,
  LifeModelCurrentView,
  LifeStateProjection,
  Model4DCompletion,
  TierStats,
} from "../../tauri";
import type { LifeModel } from "../../types";
import type { BuildLifeModelViewModelInput } from "./lifeModelViewModelAdapter";

export function makeLifeModel(overrides: Partial<LifeModel> = {}): LifeModel {
  const base: LifeModel = {
    metadata: {
      version: "limited-fixture-v1",
      created_at: "2026-07-08T00:00:00.000Z",
      updated_at: "2026-07-09T00:00:00.000Z",
      author: "fixture",
    },
    identity: {
      name: "Taylor",
      values: [
        {
          name: "Focus",
          weight: 0.8,
          description: "Protect deep work time.",
        },
      ],
      personality_traits: [],
      life_philosophy: "Build with evidence.",
      mission_statement: "Use OpenLife to plan and review important work.",
      role_definition: {
        primary_role: "Builder",
        secondary_roles: [],
        responsibilities: [],
        boundaries: [],
      },
      voice_style: {
        formality: "neutral",
        tone_descriptors: ["direct"],
        vocabulary_preference: "plain",
        emoji_usage: "never",
      },
    },
    goals: {
      short_term: [
        {
          name: "Ship the limited LifeModel slice",
          priority: 1,
          status: "in_progress",
          milestones: [],
          description: "Create a frontend-only contract adapter.",
          progress: 0.5,
          related_memories: [],
        },
      ],
      medium_term: [],
      long_term: [],
      life_goals: [],
      daily: [],
      progress: 0.5,
      related_memories: [],
    },
    capabilities: {
      skills: [
        {
          name: "Contract testing",
          proficiency: 0.7,
          description: "Uses fixtures to guard product state contracts.",
        },
      ],
      resources: [],
      networks: [],
      tools: [],
      knowledge_domains: [],
    },
    state: {
      current_focus: "Frontend ViewModel contracts",
      health_status: {
        physical: "unknown",
        mental: "unknown",
        energy_level: 0,
      },
      emotional_state: {
        current_mood: "unknown",
        stress_level: 0,
        fulfillment_score: 0,
      },
      recent_reflections: [],
      open_questions: [],
      focus_areas: ["ViewModel contract"],
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
        preferred_start: "09:00",
        preferred_end: "17:00",
        timezone: "Asia/Shanghai",
      },
      peak_energy_time: "morning",
      communication_style: "direct",
      learning_style: "examples",
      decision_making_style: "evidence-first",
    },
    evolution_rules: [],
  };

  return {
    ...base,
    ...overrides,
  };
}

export function makeEmptyLifeModel(): LifeModel {
  return makeLifeModel({
    identity: {
      name: "",
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
  });
}

export function makeLifeModelCurrentView(
  overrides: Partial<LifeModelCurrentView> = {}
): LifeModelCurrentView {
  return {
    path: "goals.short_term[0]",
    label: "Short-term goal",
    value: "Ship the limited LifeModel slice",
    unavailableReason: null,
    currentValueSource: "compatibility_view",
    change: null,
    ...overrides,
  };
}

export function makeModel4DCompletion(
  overrides: Partial<Model4DCompletion> = {}
): Model4DCompletion {
  return {
    identity: 62,
    goals: 70,
    capabilities: 55,
    state: 48,
    overall: 59,
    ...overrides,
  };
}

export function makeLifeStateProjection(
  overrides: Partial<LifeStateProjection> = {}
): LifeStateProjection {
  const pending = {
    pendingProposalCount: 1,
    editedProposalCount: 0,
    totalReviewRequiredCount: 1,
    highRiskReviewRequiredCount: 0,
    proposalStoreStatus: "ok",
    requiresUserAction: true,
    ...overrides.pending,
  };
  const readiness = {
    chatReady: true,
    usageReady: true,
    lifeModelReady: true,
    modelEmpty: false,
    pendingBuilderReviewSessions: 0,
    unfinishedBuilderSessions: 0,
    databaseStatus: "ok",
    readinessIssues: [],
    usageReadinessIssues: [],
    ...overrides.readiness,
  };
  const taskState = {
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
    ...overrides.taskState,
  };
  const safeMode = {
    active: false,
    reason: "System is not in Safe Mode.",
    sourceRefs: [],
    ...overrides.safeMode,
  };
  const toolPermissions = {
    totalCount: 0,
    activeCount: 0,
    consumedCount: 0,
    allowCount: 0,
    denyCount: 0,
    askEveryTimeCount: 0,
    allowOnceCount: 0,
    allowUntilRevokedCount: 0,
    ...overrides.toolPermissions,
  };

  return {
    version: overrides.version ?? "life_state_projection_v1",
    generatedAt: overrides.generatedAt ?? "2026-07-09T00:00:00.000Z",
    pending,
    readiness,
    taskState,
    safeMode,
    toolPermissions,
    safePaths: overrides.safePaths ?? [],
    surfaces: overrides.surfaces ?? [
      {
        surface: "life_model",
        pendingReviewCount: pending.pendingProposalCount,
        editedReviewCount: pending.editedProposalCount,
        totalReviewRequiredCount: pending.totalReviewRequiredCount,
        readinessStatus: safeMode.active ? "blocked" : "ready",
        taskStatus: "idle",
        safeModeActive: safeMode.active,
        waitingPermissionCount: taskState.waitingPermissionCount,
        activeToolPermissionCount: toolPermissions.activeCount,
      },
    ],
    sourceRefs: overrides.sourceRefs ?? [
      "LifeStateProjection.pending",
      "LifeStateProjection.readiness",
      "LifeStateProjection.safeMode",
    ],
  };
}

export function makeTierStats(overrides: Partial<TierStats> = {}): TierStats {
  return {
    total: 14,
    tier1: 5,
    tier2: 6,
    tier3: 2,
    archived: 1,
    ...overrides,
  };
}

export function makeLifeModelProposal(overrides: Partial<AgentProposal> = {}): AgentProposal {
  return {
    id: "proposal-life-goal-1",
    proposalType: "goal_update",
    source: "chat_conversation",
    affectedPath: "goals.short_term[0]",
    before: null,
    after: {
      name: "Ship the limited LifeModel slice",
    },
    reason: "User asked OpenLife to track this goal.",
    confidence: 0.72,
    riskLevel: "medium",
    status: "pending",
    createdAt: "2026-07-09T00:00:00.000Z",
    ...overrides,
  };
}

export const readyLifeModelViewModelInput: BuildLifeModelViewModelInput = {
  lifeModel: makeLifeModel(),
  currentView: makeLifeModelCurrentView(),
  completion: makeModel4DCompletion(),
  projection: makeLifeStateProjection(),
  pendingProposals: [makeLifeModelProposal()],
  memoryCount: 14,
  tierStats: makeTierStats(),
};

export const emptyLifeModelViewModelInput: BuildLifeModelViewModelInput = {
  lifeModel: makeEmptyLifeModel(),
  currentView: null,
  completion: null,
  projection: makeLifeStateProjection({
    readiness: {
      chatReady: true,
      usageReady: true,
      lifeModelReady: false,
      modelEmpty: true,
      pendingBuilderReviewSessions: 0,
      unfinishedBuilderSessions: 0,
      databaseStatus: "ok",
      readinessIssues: [],
      usageReadinessIssues: [],
    },
    pending: {
      pendingProposalCount: 0,
      editedProposalCount: 0,
      totalReviewRequiredCount: 0,
      highRiskReviewRequiredCount: 0,
      proposalStoreStatus: "ok",
      requiresUserAction: false,
    },
  }),
  pendingProposals: [],
  memoryCount: null,
  tierStats: null,
};

export const staleLifeModelViewModelInput: BuildLifeModelViewModelInput = {
  ...readyLifeModelViewModelInput,
  stale: true,
  now: "2026-07-08T23:00:00.000Z",
};

export const safeModeLifeModelViewModelInput: BuildLifeModelViewModelInput = {
  ...readyLifeModelViewModelInput,
  projection: makeLifeStateProjection({
    safeMode: {
      active: true,
      reason: "LifeModel storage is unavailable; Safe Mode is active.",
      sourceRefs: ["LifeStateProjection.safeMode"],
    },
  }),
};

export const errorLifeModelViewModelInput: BuildLifeModelViewModelInput = {
  ...readyLifeModelViewModelInput,
  error: "LifeModel primitives failed to load.",
};
