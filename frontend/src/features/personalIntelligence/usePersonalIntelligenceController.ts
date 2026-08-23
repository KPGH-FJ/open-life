import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  DraftLifeModelV2ChangeRequest,
  DraftLifeModelV2ExportRequest,
  DraftLifeModelV2RollbackRequest,
  ReviewItem,
} from "@/tauri";
import {
  buildPersonalIntelligenceErrorSnapshot,
  type PersonalIntelligenceDataSource,
  type PersonalIntelligenceSnapshot,
} from "./personalIntelligenceDataSource";
import { personalIntelligenceReviewItems } from "./personalIntelligencePresentation";
import { productErrorCode as errorText } from "@/shared/productError";

type Announce = (message: string) => void;

function preferredPersonalIntelligenceItem(items: ReviewItem[]): ReviewItem | null {
  return (
    items.find(item => item.type !== "memory_write" && item.type !== "memory_archive") ??
    items[0] ??
    null
  );
}

function reviewStillAwaitingDecision(item: ReviewItem): boolean {
  return item.status === "pending" || item.status === "edited" || item.status === "deferred";
}

function reviewItemForProposal(
  snapshot: PersonalIntelligenceSnapshot,
  proposalId: string
): ReviewItem | null {
  return (
    personalIntelligenceReviewItems(snapshot).find(item => item.source.proposalId === proposalId) ??
    null
  );
}

export function usePersonalIntelligenceController(
  dataSource: PersonalIntelligenceDataSource | undefined,
  announce: Announce
) {
  const [snapshot, setSnapshot] = useState<PersonalIntelligenceSnapshot | null>(null);
  const [selectedItemId, setSelectedItemId] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [memoryAction, setMemoryAction] = useState<{
    memoryId: string;
    action: "correct" | "archive" | "restore" | "rollback" | "erase";
    error?: string;
  } | null>(null);
  const [lifeModelAction, setLifeModelAction] = useState<{
    kind: "change" | "rollback" | "export";
    status: "submitting" | "review_required" | "failed";
    proposalId?: string;
    error?: string;
  } | null>(null);
  const [learningAction, setLearningAction] = useState<{
    candidateId: string;
    kind: "confirm" | "stage" | "delete" | "reject" | "pause_class";
    error?: string;
  } | null>(null);
  const requestRef = useRef(0);

  useEffect(() => {
    requestRef.current += 1;
    setSnapshot(null);
    setSelectedItemId(null);
    setRefreshing(false);
    setMemoryAction(null);
    setLifeModelAction(null);
    setLearningAction(null);
    return () => {
      requestRef.current += 1;
    };
  }, [dataSource]);

  const load = useCallback(
    async (announceResult = true): Promise<PersonalIntelligenceSnapshot> => {
      const requestId = ++requestRef.current;
      setRefreshing(true);
      let next: PersonalIntelligenceSnapshot;
      try {
        next = dataSource
          ? await dataSource.loadPersonalIntelligence()
          : buildPersonalIntelligenceErrorSnapshot("durable_truth_data_source_unavailable");
      } catch (error) {
        next = buildPersonalIntelligenceErrorSnapshot(error);
      }
      if (requestId === requestRef.current) {
        setSnapshot(next);
        const items = personalIntelligenceReviewItems(next);
        setSelectedItemId(current =>
          current && items.some(item => item.id === current)
            ? current
            : (preferredPersonalIntelligenceItem(items)?.id ?? null)
        );
        setLifeModelAction(current => {
          if (!current?.proposalId) return current;
          const item = items.find(candidate => candidate.source.proposalId === current.proposalId);
          return item && !reviewStillAwaitingDecision(item) ? null : current;
        });
        setRefreshing(false);
        if (announceResult) {
          const failed = [
            next.lifeModelEnvelope.status,
            next.memoryEnvelope.status,
            next.reviewEnvelope.status,
            next.boundaryEnvelope.status,
          ].some(status => status === "error");
          announce(
            failed
              ? "长期状态读取不完整；当前不确认决定或应用结果。"
              : "LifeModel、Memory 与审核状态已从系统读模型重新核对。"
          );
        }
      }
      return next;
    },
    [announce, dataSource]
  );

  const selectedItem = useMemo(() => {
    const items = personalIntelligenceReviewItems(snapshot);
    return (
      items.find(item => item.id === selectedItemId) ?? preferredPersonalIntelligenceItem(items)
    );
  }, [selectedItemId, snapshot]);

  const selectItem = useCallback((item: ReviewItem) => setSelectedItemId(item.id), []);

  const runMemoryAction = useCallback(
    async (
      memoryId: string,
      action: "correct" | "archive" | "restore" | "rollback" | "erase",
      operation: (source: PersonalIntelligenceDataSource) => Promise<void>,
      success: string
    ): Promise<boolean> => {
      if (!dataSource || memoryAction) return false;
      setMemoryAction({ memoryId, action });
      try {
        await operation(dataSource);
        const refreshed = await load(false);
        if (refreshed.memoryEnvelope.status === "error") {
          throw new Error("memory_action_refresh_unverified");
        }
        setMemoryAction(null);
        announce(success);
        return true;
      } catch (error) {
        const reason = errorText(error);
        setMemoryAction({ memoryId, action, error: reason });
        announce(`Memory 操作未完成：${reason}`);
        return false;
      }
    },
    [announce, dataSource, load, memoryAction]
  );

  const correctMemory = useCallback(
    (memoryId: string, content: string) =>
      runMemoryAction(
        memoryId,
        "correct",
        source => source.correctMemory(memoryId, content),
        "Memory 已纠正；此前内容保留为可回滚历史。"
      ),
    [runMemoryAction]
  );
  const archiveMemory = useCallback(
    (memoryId: string) =>
      runMemoryAction(
        memoryId,
        "archive",
        source => source.archiveMemory(memoryId),
        "Memory 已归档，不再参与召回；需要时可以恢复。"
      ),
    [runMemoryAction]
  );
  const restoreMemory = useCallback(
    (memoryId: string) =>
      runMemoryAction(
        memoryId,
        "restore",
        source => source.restoreMemory(memoryId),
        "Memory 已恢复为可召回状态。"
      ),
    [runMemoryAction]
  );
  const rollbackMemory = useCallback(
    (memoryId: string, reason: string) =>
      runMemoryAction(
        memoryId,
        "rollback",
        source => source.rollbackMemory(memoryId, reason),
        "该次 Memory 变更已回滚；历史仍保留。"
      ),
    [runMemoryAction]
  );
  const privacyEraseMemory = useCallback(
    (memoryId: string) =>
      runMemoryAction(
        memoryId,
        "erase",
        source => source.privacyEraseMemory(memoryId),
        "Memory 正文与派生检索内容已经永久擦除。"
      ),
    [runMemoryAction]
  );
  const draftLifeModelOperation = useCallback(
    async (
      kind: "change" | "rollback" | "export",
      operation: (source: PersonalIntelligenceDataSource) => Promise<string>
    ): Promise<boolean> => {
      if (!dataSource || lifeModelAction?.status === "submitting") return false;
      setLifeModelAction({ kind, status: "submitting" });
      try {
        const proposalId = await operation(dataSource);
        const refreshed = await load(false);
        const createdReviewItem = reviewItemForProposal(refreshed, proposalId);
        if (createdReviewItem) setSelectedItemId(createdReviewItem.id);
        setLifeModelAction({ kind, status: "review_required", proposalId });
        announce(
          refreshed.reviewEnvelope.status === "error"
            ? "建议已经创建，但 Review 状态尚未验证；当前 LifeModel 没有改变。"
            : "建议已进入 Review；批准并成功应用前，当前 LifeModel 不会改变。"
        );
        return true;
      } catch (error) {
        const reason = errorText(error);
        setLifeModelAction({ kind, status: "failed", error: reason });
        announce(`LifeModel 操作未创建：${reason}`);
        return false;
      }
    },
    [announce, dataSource, lifeModelAction?.status, load]
  );

  const draftLifeModelChange = useCallback(
    (request: DraftLifeModelV2ChangeRequest) =>
      draftLifeModelOperation("change", source => source.draftLifeModelChange(request)),
    [draftLifeModelOperation]
  );
  const draftLifeModelRollback = useCallback(
    (request: DraftLifeModelV2RollbackRequest) =>
      draftLifeModelOperation("rollback", source => source.draftLifeModelRollback(request)),
    [draftLifeModelOperation]
  );
  const draftLifeModelExport = useCallback(
    (request: DraftLifeModelV2ExportRequest) =>
      draftLifeModelOperation("export", source => source.draftLifeModelExport(request)),
    [draftLifeModelOperation]
  );
  const deleteLifeModelLearningCandidate = useCallback(
    async (candidateId: string): Promise<boolean> => {
      if (!dataSource || learningAction) return false;
      setLearningAction({ candidateId, kind: "delete" });
      try {
        await dataSource.deleteLifeModelLearningCandidate(candidateId);
        const refreshed = await load(false);
        const stillPresent = refreshed.lifeModelEnvelope.data?.learning.candidates.some(
          candidate => candidate.id === candidateId
        );
        if (refreshed.lifeModelEnvelope.status === "error" || stillPresent) {
          throw new Error("lifemodel_learning_candidate_delete_refresh_unverified");
        }
        setLearningAction(null);
        announce("这条待验证长期信息已删除；LifeModel 和 Review 没有改变。");
        return true;
      } catch (error) {
        const reason = errorText(error);
        setLearningAction({ candidateId, kind: "delete", error: reason });
        announce(`待验证长期信息未删除：${reason}`);
        return false;
      }
    },
    [announce, dataSource, learningAction, load]
  );
  const confirmLifeModelLearningCandidate = useCallback(
    async (candidateId: string): Promise<boolean> => {
      if (!dataSource || learningAction) return false;
      setLearningAction({ candidateId, kind: "confirm" });
      try {
        await dataSource.confirmLifeModelLearningCandidate(candidateId);
        const refreshed = await load(false);
        const candidate = refreshed.lifeModelEnvelope.data?.learning.candidates.find(
          item => item.id === candidateId
        );
        if (
          refreshed.lifeModelEnvelope.status === "error" ||
          !candidate ||
          candidate.status !== "reviewable" ||
          !candidate.sourceKinds.includes("user_feedback")
        ) {
          throw new Error("lifemodel_learning_candidate_confirm_refresh_unverified");
        }
        setLearningAction(null);
        announce("已记录“这条符合我”；它仍只是候选，没有创建 Proposal 或修改 LifeModel。");
        return true;
      } catch (error) {
        const reason = errorText(error);
        setLearningAction({ candidateId, kind: "confirm", error: reason });
        announce(`候选反馈未记录：${reason}`);
        return false;
      }
    },
    [announce, dataSource, learningAction, load]
  );
  const stageLifeModelLearningCandidate = useCallback(
    async (candidateId: string): Promise<boolean> => {
      if (!dataSource || learningAction) return false;
      setLearningAction({ candidateId, kind: "stage" });
      try {
        const proposalId = await dataSource.stageLifeModelLearningCandidate(candidateId);
        const refreshed = await load(false);
        const candidateStillActive = refreshed.lifeModelEnvelope.data?.learning.candidates.some(
          candidate => candidate.id === candidateId
        );
        const reviewItem = reviewItemForProposal(refreshed, proposalId);
        if (
          refreshed.lifeModelEnvelope.status === "error" ||
          refreshed.reviewEnvelope.status === "error" ||
          candidateStillActive ||
          !reviewItem ||
          !reviewStillAwaitingDecision(reviewItem)
        ) {
          throw new Error("lifemodel_learning_stage_refresh_unverified");
        }
        setSelectedItemId(reviewItem.id);
        setLearningAction(null);
        announce("这条长期信息已进入 Review Center；确认应用前，LifeModel 仍保持不变。");
        return true;
      } catch (error) {
        const reason = errorText(error);
        setLearningAction({ candidateId, kind: "stage", error: reason });
        announce(`长期信息未进入审核：${reason}`);
        return false;
      }
    },
    [announce, dataSource, learningAction, load]
  );
  const rejectLifeModelLearningCandidate = useCallback(
    async (candidateId: string): Promise<boolean> => {
      if (!dataSource || learningAction) return false;
      setLearningAction({ candidateId, kind: "reject" });
      try {
        await dataSource.rejectLifeModelLearningCandidate(candidateId);
        const refreshed = await load(false);
        const stillPresent = refreshed.lifeModelEnvelope.data?.learning.candidates.some(
          candidate => candidate.id === candidateId
        );
        if (refreshed.lifeModelEnvelope.status === "error" || stillPresent) {
          throw new Error("lifemodel_learning_candidate_reject_refresh_unverified");
        }
        setLearningAction(null);
        announce("已拒绝并清除正文；类似内容不会再次建议，LifeModel 和 Review 没有改变。");
        return true;
      } catch (error) {
        const reason = errorText(error);
        setLearningAction({ candidateId, kind: "reject", error: reason });
        announce(`候选未被拒绝：${reason}`);
        return false;
      }
    },
    [announce, dataSource, learningAction, load]
  );
  const pauseLifeModelLearningSuggestionClass = useCallback(
    async (candidateId: string): Promise<boolean> => {
      if (!dataSource || learningAction) return false;
      setLearningAction({ candidateId, kind: "pause_class" });
      try {
        await dataSource.pauseLifeModelLearningSuggestionClass(candidateId);
        const refreshed = await load(false);
        const stillPresent = refreshed.lifeModelEnvelope.data?.learning.candidates.some(
          candidate => candidate.id === candidateId
        );
        if (refreshed.lifeModelEnvelope.status === "error" || stillPresent) {
          throw new Error("lifemodel_learning_suggestion_class_pause_refresh_unverified");
        }
        setLearningAction(null);
        announce("这类长期信息建议已暂停；当前正文已清除，LifeModel 和 Review 没有改变。");
        return true;
      } catch (error) {
        const reason = errorText(error);
        setLearningAction({ candidateId, kind: "pause_class", error: reason });
        announce(`这类建议未暂停：${reason}`);
        return false;
      }
    },
    [announce, dataSource, learningAction, load]
  );

  return {
    snapshot,
    selectedItem,
    refreshing,
    memoryAction,
    lifeModelAction,
    learningAction,
    load,
    selectItem,
    correctMemory,
    archiveMemory,
    restoreMemory,
    rollbackMemory,
    privacyEraseMemory,
    draftLifeModelChange,
    draftLifeModelRollback,
    draftLifeModelExport,
    confirmLifeModelLearningCandidate,
    stageLifeModelLearningCandidate,
    deleteLifeModelLearningCandidate,
    rejectLifeModelLearningCandidate,
    pauseLifeModelLearningSuggestionClass,
  };
}
