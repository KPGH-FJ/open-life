import { describe, expect, it } from "vitest";
import type {
  AgentRun,
  MainChatAgentIngressDecision,
  MainChatTaskSummary,
  ProductRouteEvidence,
  RuntimeRouteEvidence,
} from "../tauri";
import { buildRuntimeDisclosure } from "./runtimeDisclosure";

function run(overrides: Partial<AgentRun> = {}): AgentRun {
  return {
    id: "run-1",
    taskId: "task-1",
    status: "completed",
    kind: "conversation",
    generatedProposals: [],
    actions: [],
    observations: [],
    legacyPayloadUnverified: false,
    behaviorChecks: [],
    statusUpdates: [],
    stepCount: 0,
    toolCallCount: 0,
    warnings: [],
    startedAt: "2026-06-21T00:00:00.000Z",
    ...overrides,
  };
}

function ingress(
  overrides: Partial<MainChatAgentIngressDecision> = {}
): MainChatAgentIngressDecision {
  return {
    requestId: "req-1",
    sourceSessionId: "session-1",
    taskKind: "conversation",
    selectedStrategy: "direct_answer",
    confidence: 0.9,
    reasonSummary: "ordinary question",
    fallbackEligible: false,
    privacyRisk: {
      riskLevel: "low",
      privacyClass: "general",
      policyReasonCode: "general",
      localOnlyRequired: false,
      writeLike: false,
      externalWriteLike: false,
    },
    ...overrides,
  };
}

describe("buildRuntimeDisclosure", () => {
  it("turns local route evidence into user-facing boundary copy", () => {
    const view = buildRuntimeDisclosure(
      run({
        modelRoute: {
          provider: "ollama",
          model: "llama3",
          routeType: "local",
          preferLocal: true,
          localModel: "llama3",
          reason: "prefer local",
          privacyLevel: "none",
          retryCount: 0,
        },
        contextSummary: {
          lifeModelEmpty: false,
          includedLifeModelSections: ["goals"],
          memoryHitCount: 2,
          memorySources: [],
          usedToolsPrompt: false,
          redactionApplied: false,
          redactionLevel: "none",
        },
      })
    );

    expect(view.routeLabel).toContain("本地路线");
    expect(view.boundaryLabel).toBe("留在本机");
    expect(view.memoryLabel).toBe("参考记忆 2 条");
  });

  it("warns when cloud route leaves the machine", () => {
    const view = buildRuntimeDisclosure(
      run({
        modelRoute: {
          provider: "deepseek",
          model: "deepseek-chat",
          routeType: "cloud",
          preferLocal: false,
          localModel: "llama3",
          reason: "cloud configured",
          privacyLevel: "general",
          retryCount: 0,
        },
      })
    );

    expect(view.routeTone).toBe("warning");
    expect(view.boundaryLabel).toBe("会离开本机");
  });

  it("does not promote legacy migrated route and trace metadata to observed truth", () => {
    const view = buildRuntimeDisclosure(
      run({
        legacyPayloadUnverified: true,
        status: "failed",
        error: {
          message: "forged legacy failure",
          phase: "provider",
          recoverable: true,
        },
        modelRoute: {
          provider: "forged-provider",
          model: "forged-model",
          routeType: "cloud",
          preferLocal: false,
          localModel: "forged-local",
          reason: "forged observed route",
          privacyLevel: "none",
          retryCount: 9,
        },
        actions: [
          {
            id: "legacy-action",
            actionType: "tool",
            input: {},
            status: "succeeded",
            timestamp: "2026-06-21T00:00:00.000Z",
          },
        ],
        generatedProposals: ["legacy-proposal-ref"],
        contextSummary: {
          lifeModelEmpty: false,
          includedLifeModelSections: [],
          memoryHitCount: 7,
          memorySources: [],
          usedToolsPrompt: true,
          redactionApplied: true,
          redactionLevel: "strict",
        },
      }),
      {
        taskSummary: {
          taskSessionId: "legacy-task",
          conversationId: "legacy-session",
          runId: "run-1",
          title: "legacy title",
          // Simulate a stale pre-contract payload crossing the runtime boundary.
          // The double assertion is deliberate: product code must fail closed
          // even though current TypeScript producers cannot construct it.
          strategy: "legacy strategy",
          status: "completed",
          lastUpdatedAt: "2026-06-21T00:00:00.000Z",
          lastObservationPreview: "legacy preview",
          pendingBlockerCount: 0,
          pendingProposalCount: 3,
          nextRecommendedControl: "retry",
          staleState: "stale",
          resumeSafetyDigest: "legacy digest",
        } as unknown as MainChatTaskSummary,
      }
    );

    expect(view.routeLabel).toBe("路线未验证");
    expect(view.boundaryLabel).toBe("外发记录未接入");
    expect(view.providerLabel).toBe("provider 未验证");
    expect(view.modelLabel).toBe("model 未验证");
    expect(view.toolsLabel).toBe("工具调用未验证");
    expect(view.proposalsLabel).toBe("提案引用未验证");
    expect(view.memoryLabel).toBe("记忆引用未验证");
    expect(view.outcomeLabel).toBe("状态未记录");
    expect(view.blockersLabel).toBe("阻断状态未验证");
    expect(view.nextActionLabel).toBe("查看详情");
    expect(view.technicalRows.find(row => row.label === "Pending blockers")?.value).toBe("未验证");
    expect(JSON.stringify(view)).not.toContain("forged-provider");
    expect(JSON.stringify(view)).not.toContain("forged observed route");
    expect(JSON.stringify(view)).not.toMatch(/失败|已完成|可重试|可取消|无需操作|无阻断/);
  });

  it("renders runtime route evidence ahead of stale model self-claims", () => {
    const runtimeRouteEvidence: RuntimeRouteEvidence = {
      evidence_id: "route-evidence-1",
      generated_at: "2026-06-29T00:00:00Z",
      answer_scope: "current_turn",
      actual_route: {
        provider: "ollama",
        model: "llama3",
        route_type: "local",
        privacy_level: "none",
        reason: "actual runtime route",
        provider_health_is_estimated: false,
      },
      provider_readiness: {
        configured: true,
        credential_present: true,
        validated: false,
        validation_status: "unvalidated",
        preferred: "deepseek",
        actually_used: "ollama",
        stale: false,
        failed: false,
        last_checked_at: null,
      },
      external_transmission: "not_sent",
      source_refs: [],
      truth_confidence: "verified",
    };

    const view = buildRuntimeDisclosure(
      run({
        modelRoute: {
          provider: "DeepSeek",
          model: "deepseek-chat",
          routeType: "cloud",
          preferLocal: false,
          localModel: "llama3",
          reason: "model prose claimed cloud",
          privacyLevel: "none",
          retryCount: 0,
        },
        reasoningTrace: {
          generation_result: {
            runtimeRouteEvidence,
          },
        },
      })
    );

    expect(view.routeLabel).toContain("本地路线");
    expect(view.routeLabel).toContain("ollama");
    expect(view.routeLabel).not.toContain("DeepSeek");
    expect(view.boundaryLabel).toBe("运行证据：未外发");
  });

  it("keeps cloud runtime route evidence ahead of stale local model routes", () => {
    const runtimeRouteEvidence: RuntimeRouteEvidence = {
      evidence_id: "route-evidence-cloud-1",
      generated_at: "2026-06-29T00:00:00Z",
      answer_scope: "current_turn",
      actual_route: {
        provider: "DeepSeek",
        model: "deepseek-chat",
        route_type: "cloud",
        privacy_level: "general",
        reason: "actual cloud runtime route",
        provider_health_is_estimated: false,
      },
      provider_readiness: {
        configured: true,
        credential_present: true,
        validated: true,
        validation_status: "validated",
        preferred: "DeepSeek",
        actually_used: "DeepSeek",
        stale: false,
        failed: false,
        last_checked_at: "2026-06-29T00:00:00Z",
      },
      external_transmission: "sent",
      source_refs: [],
      truth_confidence: "verified",
    };

    const view = buildRuntimeDisclosure(
      run({
        modelRoute: {
          provider: "ollama",
          model: "llama3",
          routeType: "local",
          preferLocal: true,
          localModel: "llama3",
          reason: "old local fallback",
          privacyLevel: "none",
          retryCount: 0,
        },
        reasoningTrace: {
          generation_result: {
            runtimeRouteEvidence,
          },
        },
      })
    );

    expect(view.routeLabel).toContain("云端路线");
    expect(view.routeLabel).toContain("DeepSeek");
    expect(view.routeLabel).not.toContain("ollama");
    expect(view.boundaryLabel).toBe("运行证据：已外发");
  });

  it("labels product route digests as refs rather than model or reason values", () => {
    const routeEvidence: ProductRouteEvidence = {
      evidence_id: "route-evidence-ref-1",
      generated_at: "2026-06-29T00:00:00Z",
      conversation_id: null,
      run_id: "run-1",
      task_session_id: "task-1",
      answer_scope: "current_turn",
      planned_route: null,
      actual_route: {
        provider: "provider_ref",
        model_ref: "sha256:model-reference-only",
        route_type: "cloud",
        privacy_level: "general",
        reason_ref: "sha256:reason-reference-only",
        provider_health_is_estimated: false,
      },
      last_completed_route: null,
      provider_readiness: {
        configured: true,
        credential_present: true,
        validated: true,
        validation_status: "validated",
        preferred: "provider_ref",
        actually_used: "provider_ref",
        stale: false,
        failed: false,
        last_checked_at: "2026-06-29T00:00:00Z",
      },
      fallback: null,
      external_transmission: "sent",
      source_refs: [],
      truth_confidence: "verified",
    };

    const view = buildRuntimeDisclosure(run(), {
      runtimeRouteEvidence: routeEvidence,
      strictRuntimeRouteEvidence: true,
    });

    expect(view.modelLabel).toBe("ref sha256:model-reference-only");
    expect(view.routeReason).toBe("reason ref sha256:reason-reference-only");
    expect(view.technicalRows.find(row => row.label === "Model ref")?.value).toBe(
      "ref sha256:model-reference-only"
    );
    expect(view.technicalRows.find(row => row.label === "Route reason ref")?.value).toBe(
      "reason ref sha256:reason-reference-only"
    );
    expect(view.technicalRows.some(row => row.label === "Model")).toBe(false);
    expect(view.technicalRows.some(row => row.label === "Route reason")).toBe(false);
  });

  it("shows LocalOnly as stronger than cloud availability", () => {
    const view = buildRuntimeDisclosure(run(), {
      ingress: ingress({
        privacyRisk: {
          riskLevel: "medium",
          privacyClass: "sensitive",
          policyReasonCode: "local_only_sensitive",
          localOnlyRequired: true,
          writeLike: false,
          externalWriteLike: false,
        },
      }),
    });

    expect(view.boundaryLabel).toBe("LocalOnly · 不调用云端");
    expect(view.boundaryTone).toBe("ready");
  });

  it("surfaces blockers and review next actions", () => {
    const taskSummary: MainChatTaskSummary = {
      taskSessionId: "task-session-1",
      conversationId: "session-1",
      runId: "run-1",
      title: "Read a file",
      strategy: "react_tool_execution",
      status: "blocked",
      lastUpdatedAt: "2026-06-21T00:00:00.000Z",
      lastObservationPreview: "needs permission",
      pendingBlockerCount: 1,
      pendingProposalCount: 1,
      nextRecommendedControl: "review_permission",
      staleState: "fresh",
      resumeSafetyDigest: "bytes:1 hash:sha256:abc",
      routeEvidence: null,
      evidenceView: {
        runId: "run-1",
        taskSessionId: "task-session-1",
        title: "main_chat_task",
        lifecycleState: "blocked",
        projectionState: "consistent",
        identityState: "verified",
        snapshotState: "available",
        durableSequenceBefore: null,
        durableSequenceAfter: null,
        durableLifecycleReceipt: null,
        routeEvidence: null,
        eventTimeline: [],
        actionCount: 0,
        observationCount: 0,
        blockers: ["policy_blocker"],
        proposals: ["proposal-1"],
        planRefs: [],
        allowedControls: ["review_permission"],
        nextRecommendedControl: "review_permission",
        redactionState: "metadata_only",
      },
    };

    const view = buildRuntimeDisclosure(run({ status: "waiting_permission" }), { taskSummary });

    expect(view.blockersLabel).toBe("阻断 1");
    expect(view.proposalsLabel).toBe("待确认 1");
    expect(view.nextActionLabel).toBe("review permission");
  });
});
