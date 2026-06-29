import { describe, expect, it } from "vitest";
import type {
  AgentRun,
  MainChatAgentIngressDecision,
  MainChatTaskSummary,
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
    };

    const view = buildRuntimeDisclosure(run({ status: "waiting_permission" }), { taskSummary });

    expect(view.blockersLabel).toBe("阻断 1");
    expect(view.proposalsLabel).toBe("待确认 1");
    expect(view.nextActionLabel).toBe("review permission");
  });
});
