import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  BuilderProgress,
  BuilderSignal,
  BuilderSignalDecision,
  BuilderSummary,
  ProductAction,
  UnfinishedBuilderSession,
} from "@/tauri";
import type {
  BuilderProposalReceipt,
  LifeModelBuilderDataSource,
} from "./lifeModelBuilderDataSource";

type Announce = (message: string) => void;
type CandidateDecision = "undecided" | "accepted" | "rejected" | "edited";

export type LifeModelBuilderCandidate = {
  signal: BuilderSignal;
  decision: CandidateDecision;
  editedValue: string;
};

export type LifeModelBuilderPhase =
  | "idle"
  | "loading"
  | "starting"
  | "answering"
  | "asking"
  | "reviewing"
  | "submitting"
  | "created"
  | "error";

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function editableValue(value: unknown): string {
  if (typeof value === "string") return value;
  return JSON.stringify(value, null, 2);
}

function parseEditedValue(original: unknown, input: string): unknown {
  if (typeof original === "string") return input;
  if (typeof original === "number") {
    const value = Number(input);
    if (!Number.isFinite(value)) throw new Error("builder_edited_value_invalid_number");
    return value;
  }
  try {
    return JSON.parse(input);
  } catch {
    throw new Error("builder_edited_value_invalid_json");
  }
}

export type LifeModelBuilderController = {
  phase: LifeModelBuilderPhase;
  unfinished: UnfinishedBuilderSession[];
  sessionId: string | null;
  prompt: string;
  answerDraft: string;
  progress: BuilderProgress | null;
  summary: BuilderSummary | null;
  candidates: LifeModelBuilderCandidate[];
  receipt: BuilderProposalReceipt | null;
  error: string | null;
  busy: boolean;
  ensureLoaded: () => void;
  reload: () => Promise<void>;
  startAction: (disabledReason?: string) => ProductAction;
  answerAction: (disabledReason?: string) => ProductAction;
  proposalAction: (disabledReason?: string) => ProductAction;
  start: (disabledReason?: string) => Promise<void>;
  resume: (session: UnfinishedBuilderSession, disabledReason?: string) => Promise<void>;
  setAnswerDraft: (value: string) => void;
  answer: (disabledReason?: string) => Promise<void>;
  setCandidateDecision: (signalId: string, decision: CandidateDecision) => void;
  setCandidateEditedValue: (signalId: string, value: string) => void;
  createProposals: (disabledReason?: string) => Promise<void>;
  pause: () => void;
};

export function useLifeModelBuilder(
  dataSource: LifeModelBuilderDataSource | undefined,
  announce: Announce
): LifeModelBuilderController {
  const [phase, setPhase] = useState<LifeModelBuilderPhase>("idle");
  const [unfinished, setUnfinished] = useState<UnfinishedBuilderSession[]>([]);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [prompt, setPrompt] = useState("");
  const [answerDraft, setAnswerDraft] = useState("");
  const [progress, setProgress] = useState<BuilderProgress | null>(null);
  const [summary, setSummary] = useState<BuilderSummary | null>(null);
  const [candidates, setCandidates] = useState<LifeModelBuilderCandidate[]>([]);
  const [receipt, setReceipt] = useState<BuilderProposalReceipt | null>(null);
  const [error, setError] = useState<string | null>(null);
  const requestRef = useRef(0);
  const loadedRef = useRef(false);

  useEffect(() => {
    requestRef.current += 1;
    loadedRef.current = false;
    setPhase("idle");
    setUnfinished([]);
    setSessionId(null);
    setPrompt("");
    setAnswerDraft("");
    setProgress(null);
    setSummary(null);
    setCandidates([]);
    setReceipt(null);
    setError(null);
  }, [dataSource]);

  const reload = useCallback(async () => {
    const requestId = ++requestRef.current;
    setPhase("loading");
    setError(null);
    try {
      if (!dataSource) throw new Error("lifemodel_builder_data_source_unavailable");
      const sessions = await dataSource.listUnfinished();
      if (requestId !== requestRef.current) return;
      setUnfinished(sessions.filter(session => !session.review_in_progress));
      loadedRef.current = true;
      setPhase("idle");
    } catch (loadError) {
      if (requestId !== requestRef.current) return;
      loadedRef.current = false;
      setError(errorText(loadError));
      setPhase("error");
    }
  }, [dataSource]);

  const ensureLoaded = useCallback(() => {
    if (loadedRef.current || phase === "loading") return;
    void reload();
  }, [phase, reload]);

  const applyTurn = useCallback(
    (
      nextSessionId: string,
      response: Awaited<ReturnType<LifeModelBuilderDataSource["startQuick"]>>
    ) => {
      setSessionId(nextSessionId);
      setPrompt(response.prompt);
      setProgress(response.progress);
      setAnswerDraft("");
      setReceipt(null);
      const review = response.review;
      if (response.finished) {
        if (!review || review.signals.length === 0) {
          throw new Error("builder_review_read_model_missing");
        }
        setSummary(review.summary);
        setCandidates(
          review.signals.map(signal => ({
            signal,
            decision: "undecided",
            editedValue: editableValue(signal.proposed_value),
          }))
        );
        setPhase("reviewing");
        announce("构建回答已形成候选；所有候选默认未选择，尚未创建审核建议。");
      } else {
        setSummary(null);
        setCandidates([]);
        setPhase("asking");
      }
    },
    [announce]
  );

  const startAction = useCallback(
    (disabledReason?: string): ProductAction => {
      const reason =
        disabledReason?.trim() ||
        (!dataSource
          ? "LifeModel Builder 数据源不可用。"
          : ["loading", "starting", "answering", "submitting"].includes(phase)
            ? "Builder 正在处理上一项请求。"
            : undefined);
      return {
        id: "lifemodel.builder.start-quick",
        label: "开始建立 LifeModel",
        kind: "start",
        enabled: !reason,
        ...(reason ? { disabledReason: reason } : {}),
        targetRef: "LifeModel.builder.quick",
      };
    },
    [dataSource, phase]
  );

  const answerAction = useCallback(
    (disabledReason?: string): ProductAction => {
      const reason =
        disabledReason?.trim() ||
        (phase !== "asking"
          ? "当前没有等待回答的 Builder 问题。"
          : !answerDraft.trim()
            ? "先填写回答。"
            : undefined);
      return {
        id: `lifemodel.builder.answer:${sessionId ?? "unknown"}`,
        label: "继续",
        kind: "continue",
        enabled: !reason,
        ...(reason ? { disabledReason: reason } : {}),
        targetRef: sessionId ?? "builder-session:unknown",
      };
    },
    [answerDraft, phase, sessionId]
  );

  const proposalAction = useCallback(
    (disabledReason?: string): ProductAction => {
      const undecided = candidates.filter(candidate => candidate.decision === "undecided").length;
      const retained = candidates.filter(candidate => candidate.decision !== "rejected").length;
      const reason =
        disabledReason?.trim() ||
        (phase !== "reviewing"
          ? "当前没有等待提交的 Builder 候选。"
          : undecided > 0
            ? `还有 ${undecided} 个候选未决定。`
            : retained === 0
              ? "至少保留一个候选，或选择稍后处理。"
              : undefined);
      return {
        id: `lifemodel.builder.create-proposals:${sessionId ?? "unknown"}`,
        label: "创建审核建议",
        kind: "continue",
        enabled: !reason,
        ...(reason ? { disabledReason: reason } : {}),
        targetRef: sessionId ?? "builder-session:unknown",
      };
    },
    [candidates, phase, sessionId]
  );

  const start = useCallback(
    async (disabledReason?: string) => {
      const action = startAction(disabledReason);
      if (!action.enabled || !dataSource) {
        announce(`当前不能开始建立：${action.disabledReason ?? "动作不可用"}`);
        return;
      }
      const requestId = ++requestRef.current;
      const nextSessionId = crypto.randomUUID();
      setPhase("starting");
      setError(null);
      try {
        const response = await dataSource.startQuick(nextSessionId);
        if (requestId !== requestRef.current) return;
        applyTurn(nextSessionId, response);
      } catch (startError) {
        if (requestId !== requestRef.current) return;
        setError(errorText(startError));
        setPhase("error");
        announce("LifeModel 建立会话未能启动；没有创建审核建议。");
      }
    },
    [announce, applyTurn, dataSource, startAction]
  );

  const resume = useCallback(
    async (session: UnfinishedBuilderSession, disabledReason?: string) => {
      const action = startAction(disabledReason);
      if (!action.enabled || !dataSource) {
        announce(`当前不能继续建立：${action.disabledReason ?? "动作不可用"}`);
        return;
      }
      const requestId = ++requestRef.current;
      setPhase("starting");
      setError(null);
      try {
        const response = await dataSource.resume(session);
        if (requestId !== requestRef.current) return;
        applyTurn(session.session_id, response);
      } catch (resumeError) {
        if (requestId !== requestRef.current) return;
        setError(errorText(resumeError));
        setPhase("error");
        announce("未完成的建立会话无法恢复；没有创建审核建议。");
      }
    },
    [announce, applyTurn, dataSource, startAction]
  );

  const answer = useCallback(
    async (disabledReason?: string) => {
      const action = answerAction(disabledReason);
      if (!action.enabled || !dataSource || !sessionId) {
        announce(`当前不能继续：${action.disabledReason ?? "动作不可用"}`);
        return;
      }
      const requestId = ++requestRef.current;
      setPhase("answering");
      setError(null);
      try {
        const response = await dataSource.answer(sessionId, answerDraft.trim());
        if (requestId !== requestRef.current) return;
        applyTurn(sessionId, response);
      } catch (answerError) {
        if (requestId !== requestRef.current) return;
        setError(errorText(answerError));
        setPhase("error");
        announce("Builder 回答未能提交；没有创建审核建议。");
      }
    },
    [answerAction, answerDraft, announce, applyTurn, dataSource, sessionId]
  );

  const setCandidateDecision = useCallback((signalId: string, decision: CandidateDecision) => {
    setError(null);
    setCandidates(current =>
      current.map(candidate =>
        candidate.signal.id === signalId ? { ...candidate, decision } : candidate
      )
    );
  }, []);

  const setCandidateEditedValue = useCallback((signalId: string, value: string) => {
    setError(null);
    setCandidates(current =>
      current.map(candidate =>
        candidate.signal.id === signalId
          ? { ...candidate, decision: "edited", editedValue: value }
          : candidate
      )
    );
  }, []);

  const createProposals = useCallback(
    async (disabledReason?: string) => {
      const action = proposalAction(disabledReason);
      if (!action.enabled || !dataSource || !sessionId) {
        announce(`当前不能创建审核建议：${action.disabledReason ?? "动作不可用"}`);
        return;
      }
      let decisions: BuilderSignalDecision[];
      try {
        decisions = candidates.map(candidate => {
          if (candidate.decision === "undecided") {
            throw new Error("builder_candidate_decision_missing");
          }
          return {
            id: candidate.signal.id,
            status: candidate.decision,
            ...(candidate.decision === "edited"
              ? {
                  proposed_value: parseEditedValue(
                    candidate.signal.proposed_value,
                    candidate.editedValue
                  ),
                }
              : {}),
          };
        });
      } catch (decisionError) {
        setError(errorText(decisionError));
        announce("候选修改值格式无效；没有创建审核建议。");
        return;
      }
      const requestId = ++requestRef.current;
      setPhase("submitting");
      setError(null);
      try {
        const nextReceipt = await dataSource.createProposals(sessionId, decisions);
        if (requestId !== requestRef.current) return;
        if (!nextReceipt.success) throw new Error("builder_proposal_receipt_not_successful");
        setReceipt(nextReceipt);
        setPhase("created");
        setUnfinished(current => current.filter(session => session.session_id !== sessionId));
        announce("审核建议已创建；它们尚未批准，也尚未写入 LifeModel。");
      } catch (submitError) {
        if (requestId !== requestRef.current) return;
        setError(errorText(submitError));
        setPhase("reviewing");
        announce("审核建议创建失败；LifeModel 没有因此改变。");
      }
    },
    [announce, candidates, dataSource, proposalAction, sessionId]
  );

  const pause = useCallback(() => {
    requestRef.current += 1;
    setSessionId(null);
    setPrompt("");
    setAnswerDraft("");
    setProgress(null);
    setSummary(null);
    setCandidates([]);
    setReceipt(null);
    setError(null);
    loadedRef.current = false;
    setPhase("idle");
    announce("已暂停建立；没有创建审核建议，也没有写入 LifeModel。");
    void reload();
  }, [announce, reload]);

  const busy = ["loading", "starting", "answering", "submitting"].includes(phase);

  return useMemo(
    () => ({
      phase,
      unfinished,
      sessionId,
      prompt,
      answerDraft,
      progress,
      summary,
      candidates,
      receipt,
      error,
      busy,
      ensureLoaded,
      reload,
      startAction,
      answerAction,
      proposalAction,
      start,
      resume,
      setAnswerDraft,
      answer,
      setCandidateDecision,
      setCandidateEditedValue,
      createProposals,
      pause,
    }),
    [
      answer,
      answerAction,
      answerDraft,
      busy,
      candidates,
      createProposals,
      ensureLoaded,
      error,
      pause,
      phase,
      progress,
      prompt,
      proposalAction,
      receipt,
      reload,
      resume,
      sessionId,
      start,
      startAction,
      summary,
      unfinished,
    ]
  );
}
