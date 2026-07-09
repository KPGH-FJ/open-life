import type {
  AgentProposal,
  LifeModelCurrentView,
  LifeStateProjection,
  Model4DCompletion,
  TierStats,
} from "../../tauri";
import type { LifeModel } from "../../types";
import type {
  DebugAction,
  EvidenceRef,
  ProductAction,
  ViewModelEnvelope,
  ViewModelStatus,
  ViewModelWarning,
} from "../shared/viewModelEnvelope";
import type {
  LifeModelCandidateChange,
  LifeModelConfidence,
  LifeModelCurrentViewSummary,
  LifeModelDimensionId,
  LifeModelDimensionSummary,
  LifeModelManualOverrideState,
  LifeModelMemoryLinkageSummary,
  LifeModelPendingUpdateCounts,
  LifeModelReviewItemRef,
  LifeModelTruthMode,
  LifeModelTrustQualityState,
  LifeModelViewModel,
  LifeModelViewModelEnvelope,
} from "./lifeModelViewModel";

export type BuildLifeModelViewModelInput = {
  lifeModel: LifeModel | null;
  currentView: LifeModelCurrentView | null;
  completion: Model4DCompletion | null;
  projection: LifeStateProjection | null;
  pendingProposals: AgentProposal[];
  memoryCount: number | null;
  tierStats: TierStats | null;
  now?: string;
  stale?: boolean;
  error?: string | null;
};

const BACKEND_SOURCE = "backend-readmodel" as const;
const LIFE_MODEL_TARGET_REF = "lifemodel";
const REFRESH_DISABLED_REASON = "LifeModel state is still loading.";
const STALE_DISABLED_REASON = "Refresh LifeModel state before using this action.";
const SAFE_MODE_DISABLED_REASON =
  "LifeStateProjection reports Safe Mode; risky actions are disabled.";
const PHASE_2_DISABLED_REASON =
  "PHASE_2_REQUIRED: LifeModel update actions require a backend-owned LifeModelViewModel and ReviewWorkflow.";

const DIMENSIONS: Array<{ id: LifeModelDimensionId; label: string }> = [
  { id: "identity", label: "Identity" },
  { id: "goals", label: "Goals" },
  { id: "capabilities", label: "Capabilities" },
  { id: "state", label: "State" },
];

export function buildLifeModelViewModelEnvelope(
  input: BuildLifeModelViewModelInput
): LifeModelViewModelEnvelope {
  const now = input.now ?? input.projection?.generatedAt ?? null;

  if (input.error) {
    return buildNullEnvelope({
      status: "error",
      lastUpdatedAt: now,
      warnings: [
        {
          code: "lifemodel.load_error",
          message:
            "LifeModelViewModel could not load required primitives; no raw LifeModel fallback was used.",
          severity: "error",
        },
      ],
      primaryActions: [refreshAction(true)],
    });
  }

  const sourceRefs = collectSourceRefs(input);
  const lifeModelProposals = input.pendingProposals.filter(isLifeModelProposal);
  const loadedStatus = deriveLoadedStatus(input);
  const riskyActionsDisabledReason = riskyActionDisabledReason(input, loadedStatus);
  const pendingUpdateCounts = buildPendingUpdateCounts(lifeModelProposals);
  const relatedReviewItemRefs = lifeModelProposals.map(reviewItemRefFromProposal);
  const currentViewSummary = buildCurrentViewSummary(input, sourceRefs);
  const truthMode = deriveTruthMode(input, currentViewSummary);
  const dimensionSummaries = buildDimensionSummaries({
    lifeModel: input.lifeModel,
    completion: input.completion,
    proposals: lifeModelProposals,
    sourceRefs,
    stale: loadedStatus === "stale",
  });
  const warnings = buildLoadedWarnings({
    status: loadedStatus,
    input,
    sourceRefs,
    acceptedProposalCount: pendingUpdateCounts.approvedNotApplied,
  });

  const data: LifeModelViewModel = {
    truthMode,
    canonicalSummary: null,
    currentViewSummary,
    dimensionSummaries,
    trustQualityState: buildTrustQualityState({
      status: loadedStatus,
      input,
      dimensionSummaries,
      sourceRefs,
    }),
    pendingUpdateCounts,
    provenanceRefs: sourceRefs,
    candidateChanges: lifeModelProposals
      .filter(proposal => proposal.status === "pending")
      .map(candidateChangeFromProposal),
    materializedChanges: [],
    manualOverrideState: buildManualOverrideState(sourceRefs),
    relatedReviewItemRefs,
    memoryLinkage: buildMemoryLinkage(input, sourceRefs),
    sourceRefs,
    contractLimitations: [
      "No backend-owned LifeModelViewModel owner exists in this limited slice.",
      "Raw LifeModel primitives are labeled current/compatibility, not canonical truth.",
      "Accepted proposal decisions are not treated as durable LifeModel materialization.",
      "Memory linkage remains partial until a backend Memory/LifeModel read model exists.",
    ],
  };

  return {
    data,
    status: loadedStatus,
    lastUpdatedAt: now,
    source: BACKEND_SOURCE,
    evidenceRefs: sourceRefs,
    warnings,
    actions: {
      primary: [
        refreshAction(true),
        evidenceAction(
          loadedStatus !== "stale",
          loadedStatus === "stale" ? STALE_DISABLED_REASON : undefined
        ),
        requestUpdateAction(riskyActionsDisabledReason),
      ],
      review: [],
      debugOnly: debugActionsFor(sourceRefs, loadedStatus),
    },
  };
}

function buildNullEnvelope({
  status,
  lastUpdatedAt,
  warnings = [],
  primaryActions,
}: {
  status: ViewModelStatus;
  lastUpdatedAt: string | null;
  warnings?: ViewModelWarning[];
  primaryActions: ProductAction[];
}): ViewModelEnvelope<LifeModelViewModel> {
  return {
    data: null,
    status,
    lastUpdatedAt,
    source: BACKEND_SOURCE,
    evidenceRefs: [],
    warnings,
    actions: {
      primary: primaryActions,
      review: [],
      debugOnly: [],
    },
  };
}

function deriveLoadedStatus(input: BuildLifeModelViewModelInput): ViewModelStatus {
  if (input.stale) return "stale";
  if (!input.lifeModel && !input.currentView) return "empty";
  if (input.projection?.readiness.modelEmpty) return "empty";
  if (input.lifeModel && !lifeModelHasMeaningfulContent(input.lifeModel) && !input.currentView) {
    return "empty";
  }
  return "ready";
}

function deriveTruthMode(
  input: BuildLifeModelViewModelInput,
  currentViewSummary: LifeModelCurrentViewSummary | null
): LifeModelTruthMode {
  if (currentViewSummary) return "current_compatibility";
  if (input.lifeModel && lifeModelHasMeaningfulContent(input.lifeModel)) {
    return "current_compatibility";
  }
  return "unknown";
}

function buildCurrentViewSummary(
  input: BuildLifeModelViewModelInput,
  sourceRefs: EvidenceRef[]
): LifeModelCurrentViewSummary | null {
  if (input.currentView) {
    const value = input.currentView.value?.trim();
    const unavailable = input.currentView.unavailableReason?.trim();
    return {
      currentViewRef: {
        id: `lifemodel-current:${input.currentView.path}`,
        kind: "lifemodel",
        label: input.currentView.label || input.currentView.path,
      },
      compatibilityMode: true,
      label: input.currentView.label || input.currentView.path,
      summary: value || unavailable || "Current compatibility view loaded without a display value.",
      divergenceFromCanonical: "unknown",
      evidenceRefs: sourceRefs,
      ownerStatus: "PARTIAL",
    };
  }

  if (input.lifeModel && lifeModelHasMeaningfulContent(input.lifeModel)) {
    return {
      currentViewRef: {
        id: `lifemodel-current:${input.lifeModel.metadata.version || "unknown"}`,
        kind: "lifemodel",
        label: "Existing LifeModel primitive",
      },
      compatibilityMode: true,
      label: "Existing LifeModel primitive",
      summary:
        "A raw LifeModel primitive is available, but this limited slice does not label it canonical truth.",
      divergenceFromCanonical: "unknown",
      evidenceRefs: sourceRefs,
      ownerStatus: "PARTIAL",
    };
  }

  return null;
}

function buildDimensionSummaries({
  lifeModel,
  completion,
  proposals,
  sourceRefs,
  stale,
}: {
  lifeModel: LifeModel | null;
  completion: Model4DCompletion | null;
  proposals: AgentProposal[];
  sourceRefs: EvidenceRef[];
  stale: boolean;
}): LifeModelDimensionSummary[] {
  if (!lifeModel || !lifeModelHasMeaningfulContent(lifeModel)) return [];

  return DIMENSIONS.map(dimension => {
    const dimensionProposals = proposals.filter(proposal =>
      affectedDimensionIds(proposal).includes(dimension.id)
    );
    return {
      id: dimension.id,
      label: dimension.label,
      summary: dimensionSummary(lifeModel, dimension.id),
      confidence: confidenceFromCompletion(completion?.[dimension.id]),
      stale,
      pendingReviewItemRefs: dimensionProposals
        .filter(proposal => proposal.status === "pending")
        .map(reviewItemRefFromProposal),
      evidenceRefs: uniqueEvidenceRefs([
        ...sourceRefs,
        ...dimensionProposals.map(evidenceRefFromProposal),
      ]),
      provenance: "limited",
      ownerStatus: "PHASE_2_REQUIRED",
    };
  });
}

function buildTrustQualityState({
  status,
  input,
  dimensionSummaries,
  sourceRefs,
}: {
  status: ViewModelStatus;
  input: BuildLifeModelViewModelInput;
  dimensionSummaries: LifeModelDimensionSummary[];
  sourceRefs: EvidenceRef[];
}): LifeModelTrustQualityState {
  const missingDimensionCount = DIMENSIONS.length - dimensionSummaries.length;
  const staleDimensionCount = dimensionSummaries.filter(dimension => dimension.stale).length;

  if (status === "stale") {
    return {
      readiness: "stale",
      completionScore: input.completion?.overall ?? null,
      missingDimensionCount,
      staleDimensionCount,
      warningRefs: sourceRefs,
      ownerStatus: "PHASE_2_REQUIRED",
    };
  }

  if (status === "empty") {
    return {
      readiness: "not_built",
      completionScore: input.completion?.overall ?? null,
      missingDimensionCount: DIMENSIONS.length,
      staleDimensionCount: 0,
      warningRefs: sourceRefs,
      ownerStatus: "PHASE_2_REQUIRED",
    };
  }

  if (
    input.projection &&
    (!input.projection.readiness.lifeModelReady ||
      input.projection.readiness.readinessIssues.length > 0 ||
      input.projection.readiness.usageReadinessIssues.length > 0)
  ) {
    return {
      readiness: "limited",
      completionScore: input.completion?.overall ?? null,
      missingDimensionCount,
      staleDimensionCount,
      warningRefs: sourceRefs,
      ownerStatus: "PHASE_2_REQUIRED",
    };
  }

  return {
    readiness: "usable_with_limits",
    completionScore: input.completion?.overall ?? null,
    missingDimensionCount,
    staleDimensionCount,
    warningRefs: sourceRefs,
    ownerStatus: "PHASE_2_REQUIRED",
  };
}

function buildPendingUpdateCounts(proposals: AgentProposal[]): LifeModelPendingUpdateCounts {
  return {
    candidate: proposals.filter(proposal => proposal.status === "pending").length,
    pendingReview: proposals.filter(proposal => proposal.status === "pending").length,
    approvedNotApplied: proposals.filter(proposal => proposal.status === "accepted").length,
    failedMaterialization: 0,
    ownerStatus: "PHASE_2_REQUIRED",
  };
}

function candidateChangeFromProposal(proposal: AgentProposal): LifeModelCandidateChange {
  return {
    changeRef: {
      id: `proposal:${proposal.id}`,
      kind: "proposal",
      label: proposal.reason || proposal.affectedPath || proposal.id,
    },
    title: proposal.reason || `Pending LifeModel change for ${proposal.affectedPath}`,
    changeKind: changeKindFromProposal(proposal),
    affectedDimensionIds: affectedDimensionIds(proposal),
    reviewItemRefs: [reviewItemRefFromProposal(proposal)],
    evidenceRefs: [evidenceRefFromProposal(proposal)],
    decisionStatus: "pending",
  };
}

function buildManualOverrideState(sourceRefs: EvidenceRef[]): LifeModelManualOverrideState {
  return {
    active: false,
    blockedReason:
      "Manual override state is PHASE_2_REQUIRED; this limited adapter exposes no save action.",
    draftRef: null,
    saveAction: null,
    reviewItemRefs: [],
    evidenceRefs: sourceRefs,
    ownerStatus: "PHASE_2_REQUIRED",
  };
}

function buildMemoryLinkage(
  input: BuildLifeModelViewModelInput,
  sourceRefs: EvidenceRef[]
): LifeModelMemoryLinkageSummary {
  const memoryEvidenceRefs = memoryEvidenceRefsFromInput(input);
  const linkedMemoryCount = input.memoryCount ?? input.tierStats?.total ?? 0;
  return {
    linkedMemoryCount,
    candidateMemoryCount: 0,
    materializedMemoryCount: 0,
    conflictCount: 0,
    memoryRefs: [],
    evidenceRefs: uniqueEvidenceRefs([...sourceRefs, ...memoryEvidenceRefs]),
    linkageStatus: input.memoryCount === null && !input.tierStats ? "unknown" : "partial",
    tierSummary: {
      total: input.tierStats?.total ?? null,
      tier1: input.tierStats?.tier1 ?? null,
      tier2: input.tierStats?.tier2 ?? null,
      tier3: input.tierStats?.tier3 ?? null,
      archived: input.tierStats?.archived ?? null,
    },
    ownerStatus: "PHASE_2_REQUIRED",
  };
}

function buildLoadedWarnings({
  status,
  input,
  sourceRefs,
  acceptedProposalCount,
}: {
  status: ViewModelStatus;
  input: BuildLifeModelViewModelInput;
  sourceRefs: EvidenceRef[];
  acceptedProposalCount: number;
}): ViewModelWarning[] {
  const warnings: ViewModelWarning[] = [
    {
      code: "lifemodel.truth_mode_required",
      message:
        "Canonical/current truth mode is PHASE_2_REQUIRED; loaded primitives are labeled current/compatibility.",
      severity: "warning",
      evidenceRefs: sourceRefs,
    },
    {
      code: "lifemodel.canonical_summary_unavailable",
      message:
        "Canonical LifeModel summary is unavailable because no backend-owned LifeModelViewModel was provided.",
      severity: "info",
      evidenceRefs: sourceRefs,
    },
    {
      code: "lifemodel.memory_linkage_limited",
      message:
        "Memory linkage uses only memory count and tier stats when provided; lane/materialization ownership remains PHASE_2_REQUIRED.",
      severity: "info",
      evidenceRefs: sourceRefs,
    },
  ];

  if (status === "empty") {
    warnings.push({
      code: "lifemodel.empty_limited",
      message:
        "No confirmed LifeModel content was provided; no fake canonical summary was generated.",
      severity: "info",
      evidenceRefs: sourceRefs,
    });
  }

  if (status === "stale") {
    warnings.push({
      code: "lifemodel.stale",
      message: "LifeModelViewModel data is stale; risky actions are disabled until refresh.",
      severity: "warning",
      evidenceRefs: sourceRefs,
    });
  }

  if (input.projection?.safeMode.active) {
    warnings.push({
      code: "lifemodel.safe_mode",
      message: "LifeStateProjection reports Safe Mode; risky LifeModel actions are disabled.",
      severity: "warning",
      evidenceRefs: sourceRefs,
    });
  }

  if (acceptedProposalCount > 0 || input.currentView?.change) {
    warnings.push({
      code: "lifemodel.materialization_owner_required",
      message:
        "Accepted proposal or current-view evidence is not durable materialization proof in this limited slice.",
      severity: "warning",
      evidenceRefs: sourceRefs,
    });
  }

  return warnings;
}

function riskyActionDisabledReason(
  input: BuildLifeModelViewModelInput,
  status: ViewModelStatus
): string {
  if (status === "stale") return STALE_DISABLED_REASON;
  if (input.projection?.safeMode.active) return SAFE_MODE_DISABLED_REASON;
  return PHASE_2_DISABLED_REASON;
}

function refreshAction(enabled: boolean): ProductAction {
  return {
    id: "lifemodel.refresh",
    label: "Refresh LifeModel state",
    kind: "refresh",
    enabled,
    disabledReason: enabled ? undefined : REFRESH_DISABLED_REASON,
    targetRef: LIFE_MODEL_TARGET_REF,
  };
}

function evidenceAction(enabled: boolean, disabledReason?: string): ProductAction {
  return {
    id: "lifemodel.inspect_evidence",
    label: "Inspect LifeModel evidence",
    kind: "inspect",
    enabled,
    disabledReason,
    targetRef: "lifemodel:evidence",
  };
}

function requestUpdateAction(disabledReason: string): ProductAction {
  return {
    id: "lifemodel.request_update",
    label: "Request LifeModel update",
    kind: "start",
    enabled: false,
    disabledReason,
    targetRef: LIFE_MODEL_TARGET_REF,
  };
}

function debugActionsFor(evidenceRefs: EvidenceRef[], status: ViewModelStatus): DebugAction[] {
  if (status === "empty" || evidenceRefs.length === 0) return [];
  return [
    {
      id: "lifemodel.inspect_limited_input_refs",
      label: "Inspect limited LifeModel input refs",
      kind: "raw_json",
      enabled: status !== "stale",
      developerOnly: true,
      targetRef: "LifeModelViewModel.limitedInputRefs",
    },
  ];
}

function collectSourceRefs(input: BuildLifeModelViewModelInput): EvidenceRef[] {
  return uniqueEvidenceRefs([
    ...projectionEvidenceRefs(input.projection),
    ...lifeModelEvidenceRefs(input.lifeModel),
    ...currentViewEvidenceRefs(input.currentView),
    ...completionEvidenceRefs(input.completion),
    ...input.pendingProposals.filter(isLifeModelProposal).map(evidenceRefFromProposal),
    ...memoryEvidenceRefsFromInput(input),
  ]);
}

function projectionEvidenceRefs(projection: LifeStateProjection | null): EvidenceRef[] {
  if (!projection) return [];
  if (projection.sourceRefs.length === 0) {
    return [
      {
        id: "projection:LifeStateProjection",
        label: "LifeStateProjection",
        source: BACKEND_SOURCE,
        sensitivity: "local_private",
      },
    ];
  }
  return projection.sourceRefs.map((sourceRef, index) => ({
    id: `projection:${index}:${sourceRef}`,
    label: sourceRef,
    source: BACKEND_SOURCE,
    sensitivity: "local_private",
  }));
}

function lifeModelEvidenceRefs(lifeModel: LifeModel | null): EvidenceRef[] {
  if (!lifeModel) return [];
  return [
    {
      id: `lifemodel:${lifeModel.metadata.version || "unknown"}`,
      label: `LifeModel primitive ${lifeModel.metadata.version || "unknown"}`,
      source: "lifemodel",
      sensitivity: "local_private",
    },
  ];
}

function currentViewEvidenceRefs(currentView: LifeModelCurrentView | null): EvidenceRef[] {
  if (!currentView) return [];
  return [
    {
      id: `lifemodel-current:${currentView.path}`,
      label: `LifeModelCurrentView ${currentView.path}`,
      source: "lifemodel",
      sensitivity: "local_private",
    },
  ];
}

function completionEvidenceRefs(completion: Model4DCompletion | null): EvidenceRef[] {
  if (!completion) return [];
  return [
    {
      id: "lifemodel-completion:model4d",
      label: "Model4DCompletion",
      source: "lifemodel",
      sensitivity: "local_private",
    },
  ];
}

function memoryEvidenceRefsFromInput(input: BuildLifeModelViewModelInput): EvidenceRef[] {
  const refs: EvidenceRef[] = [];
  if (input.memoryCount !== null) {
    refs.push({
      id: "memory:count",
      label: "Memory count",
      source: "memory",
      sensitivity: "local_private",
    });
  }
  if (input.tierStats) {
    refs.push({
      id: "memory:tier-stats",
      label: "Memory tier stats",
      source: "memory",
      sensitivity: "local_private",
    });
  }
  return refs;
}

function evidenceRefFromProposal(proposal: AgentProposal): EvidenceRef {
  return {
    id: `proposal:${proposal.id}`,
    label: `Proposal ${proposal.id}: ${proposal.affectedPath}`,
    source: "review",
    sensitivity: proposal.riskLevel === "critical" ? "sensitive" : "local_private",
  };
}

function reviewItemRefFromProposal(proposal: AgentProposal): LifeModelReviewItemRef {
  return {
    id: `proposal:${proposal.id}`,
    kind: "review_item",
    label: proposal.reason || proposal.affectedPath || proposal.id,
  };
}

function isLifeModelProposal(proposal: AgentProposal): boolean {
  if (
    proposal.proposalType === "life_model_update" ||
    proposal.proposalType === "goal_update" ||
    proposal.proposalType === "state_update" ||
    proposal.proposalType === "preference_update" ||
    proposal.proposalType === "capability_update"
  ) {
    return true;
  }

  const path = proposal.affectedPath.toLowerCase();
  return [
    "metadata",
    "identity",
    "goals",
    "capabilities",
    "state",
    "relationships",
    "preferences",
    "evolution_rules",
  ].some(prefix => path === prefix || path.startsWith(`${prefix}.`));
}

function changeKindFromProposal(proposal: AgentProposal): LifeModelCandidateChange["changeKind"] {
  if (proposal.before === undefined || proposal.before === null) return "add";
  if (proposal.after === undefined || proposal.after === null) return "remove";
  return "update";
}

function affectedDimensionIds(proposal: AgentProposal): string[] {
  const path = proposal.affectedPath.toLowerCase();
  const ids: string[] = DIMENSIONS.filter(dimension => path.startsWith(dimension.id)).map(
    dimension => dimension.id
  );
  if (path.startsWith("preferences")) ids.push("preferences");
  if (path.startsWith("relationships")) ids.push("relationships");
  return ids.length > 0 ? ids : ["unknown"];
}

function confidenceFromCompletion(value: number | null | undefined): LifeModelConfidence {
  if (typeof value !== "number" || Number.isNaN(value)) return "unknown";
  const normalized = value > 0 && value <= 1 ? value * 100 : value;
  if (normalized >= 75) return "high";
  if (normalized >= 40) return "medium";
  return "low";
}

function dimensionSummary(lifeModel: LifeModel, dimension: LifeModelDimensionId): string {
  switch (dimension) {
    case "identity": {
      const parts = [
        compact(lifeModel.identity.name) ? `name: ${compact(lifeModel.identity.name)}` : "",
        compact(lifeModel.identity.mission_statement)
          ? `mission: ${compact(lifeModel.identity.mission_statement)}`
          : "",
        lifeModel.identity.values.length > 0
          ? `${lifeModel.identity.values.length} value item(s)`
          : "",
      ].filter(Boolean);
      return limitedSummary(parts, "identity");
    }
    case "goals": {
      const goalCount =
        lifeModel.goals.short_term.length +
        lifeModel.goals.medium_term.length +
        lifeModel.goals.long_term.length +
        lifeModel.goals.life_goals.length +
        lifeModel.goals.daily.length;
      const parts = [
        goalCount > 0 ? `${goalCount} goal item(s)` : "",
        Number.isFinite(lifeModel.goals.progress) ? `progress: ${lifeModel.goals.progress}` : "",
      ].filter(Boolean);
      return limitedSummary(parts, "goals");
    }
    case "capabilities": {
      const parts = [
        lifeModel.capabilities.skills.length > 0
          ? `${lifeModel.capabilities.skills.length} skill(s)`
          : "",
        lifeModel.capabilities.tools.length > 0
          ? `${lifeModel.capabilities.tools.length} tool capability item(s)`
          : "",
        lifeModel.capabilities.knowledge_domains.length > 0
          ? `${lifeModel.capabilities.knowledge_domains.length} knowledge domain(s)`
          : "",
      ].filter(Boolean);
      return limitedSummary(parts, "capabilities");
    }
    case "state": {
      const parts = [
        compact(lifeModel.state.current_focus)
          ? `current focus: ${compact(lifeModel.state.current_focus)}`
          : "",
        lifeModel.state.focus_areas.length > 0
          ? `${lifeModel.state.focus_areas.length} focus area(s)`
          : "",
        lifeModel.state.alerts.length > 0 ? `${lifeModel.state.alerts.length} alert(s)` : "",
      ].filter(Boolean);
      return limitedSummary(parts, "state");
    }
  }
}

function limitedSummary(parts: string[], dimension: string): string {
  if (parts.length === 0) {
    return `No confirmed ${dimension} content was provided by the limited input.`;
  }
  return `Limited current primitive summary: ${parts.join("; ")}.`;
}

function lifeModelHasMeaningfulContent(lifeModel: LifeModel): boolean {
  return (
    compact(lifeModel.identity.name).length > 0 ||
    compact(lifeModel.identity.mission_statement).length > 0 ||
    lifeModel.identity.values.length > 0 ||
    lifeModel.goals.short_term.length > 0 ||
    lifeModel.goals.medium_term.length > 0 ||
    lifeModel.goals.long_term.length > 0 ||
    lifeModel.goals.life_goals.length > 0 ||
    lifeModel.goals.daily.length > 0 ||
    lifeModel.capabilities.skills.length > 0 ||
    lifeModel.capabilities.resources.length > 0 ||
    lifeModel.capabilities.tools.length > 0 ||
    lifeModel.capabilities.knowledge_domains.length > 0 ||
    compact(lifeModel.state.current_focus).length > 0 ||
    lifeModel.state.focus_areas.length > 0 ||
    lifeModel.state.recent_events.length > 0
  );
}

function compact(value: string | null | undefined): string {
  return value?.trim() ?? "";
}

function uniqueEvidenceRefs(refs: EvidenceRef[]): EvidenceRef[] {
  const seen = new Set<string>();
  return refs.filter(ref => {
    if (seen.has(ref.id)) return false;
    seen.add(ref.id);
    return true;
  });
}
