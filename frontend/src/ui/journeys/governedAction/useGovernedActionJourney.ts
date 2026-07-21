import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import type { ReviewAction, ReviewItem, TaskControl, TaskViewModelItem } from "@/tauri";
import {
  initialReviewDispatchState,
  reviewDispatchReducer,
  type ReviewDispatchState,
} from "@/contracts/reviewDispatchContract";
import {
  initialTaskResumeState,
  taskResumeReducer,
  type TaskResumeState,
} from "./governedActionContract";
import {
  buildGovernedActionErrorSnapshot,
  type GovernedActionDataSource,
  type GovernedActionSnapshot,
} from "./governedActionDataSource";

type Announce = (message: string) => void;

function errorCode(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function reviewEnvelopeAllowsDecisions(snapshot: GovernedActionSnapshot | null): boolean {
  return snapshot?.reviewEnvelope.status === "ready";
}

function workspaceEnvelopeAllowsControls(snapshot: GovernedActionSnapshot | null): boolean {
  return snapshot?.workspaceEnvelope.status === "ready";
}

function findRefreshedTask(
  snapshot: GovernedActionSnapshot,
  targetTaskId: string
): TaskViewModelItem | null {
  return (
    snapshot.tasksEnvelope.data?.items.find(task => task.canonicalTaskId === targetTaskId) ?? null
  );
}

export type GovernedActionJourneyController = {
  snapshot: GovernedActionSnapshot | null;
  selectedItem: ReviewItem | null;
  refreshing: boolean;
  reviewState: ReviewDispatchState;
  resumeState: TaskResumeState;
  load: (announceResult?: boolean) => Promise<GovernedActionSnapshot>;
  selectReviewItem: (itemOrId: ReviewItem | string) => void;
  requestReviewAction: (action: ReviewAction) => void;
  confirmReviewAction: () => void;
  cancelReviewConfirmation: () => void;
  requestResume: (control: TaskControl, expectedTaskId: string) => void;
  confirmResume: () => void;
  cancelResumeConfirmation: () => void;
};

export function useGovernedActionJourney(
  dataSource: GovernedActionDataSource | undefined,
  announce: Announce
): GovernedActionJourneyController {
  const [snapshot, setSnapshot] = useState<GovernedActionSnapshot | null>(null);
  const [selectedItemId, setSelectedItemId] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [reviewState, dispatchReview] = useReducer(
    reviewDispatchReducer,
    initialReviewDispatchState
  );
  const [resumeState, dispatchResume] = useReducer(taskResumeReducer, initialTaskResumeState);
  const snapshotRef = useRef<GovernedActionSnapshot | null>(null);
  const requestRef = useRef(0);
  const operationSequenceRef = useRef(0);
  const activeReviewOperationRef = useRef<number | null>(null);
  const activeResumeOperationRef = useRef<number | null>(null);

  useEffect(() => {
    requestRef.current += 1;
    activeReviewOperationRef.current = null;
    activeResumeOperationRef.current = null;
    snapshotRef.current = null;
    setSnapshot(null);
    setSelectedItemId(null);
    setRefreshing(false);
    dispatchReview({ type: "reset" });
    dispatchResume({ type: "reset" });
    return () => {
      requestRef.current += 1;
    };
  }, [dataSource]);

  const loadSnapshot = useCallback(
    async (announceResult = true): Promise<GovernedActionSnapshot> => {
      const requestId = ++requestRef.current;
      setRefreshing(true);
      let next: GovernedActionSnapshot;
      try {
        next = dataSource
          ? await dataSource.load()
          : buildGovernedActionErrorSnapshot("governed_action_data_source_unavailable");
      } catch (error) {
        next = buildGovernedActionErrorSnapshot(error);
      }
      if (requestId === requestRef.current) {
        snapshotRef.current = next;
        setSnapshot(next);
        setSelectedItemId(current => {
          const items = ["ready", "stale"].includes(next.reviewEnvelope.status)
            ? (next.reviewEnvelope.data?.items ?? [])
            : [];
          if (current && items.some(item => item.id === current)) {
            return current;
          }
          return items[0]?.id ?? null;
        });
        setRefreshing(false);
        if (announceResult) {
          const failed = [
            next.workspaceEnvelope.status,
            next.reviewEnvelope.status,
            next.tasksEnvelope.status,
          ].some(status => status === "error");
          announce(
            failed
              ? "业务状态读取不完整；审核决定与任务控制保持关闭。"
              : "工作区、审核与任务状态已经从后端读模型重新核对。"
          );
        }
      }
      return next;
    },
    [announce, dataSource]
  );

  const load = useCallback(
    async (announceResult = true): Promise<GovernedActionSnapshot> => {
      if (activeReviewOperationRef.current !== null || activeResumeOperationRef.current !== null) {
        announce("已有决定或任务请求正在核对；本次没有并发刷新。");
        return (
          snapshotRef.current ??
          buildGovernedActionErrorSnapshot("governed_action_refresh_blocked_without_snapshot")
        );
      }
      dispatchReview({ type: "reset" });
      dispatchResume({ type: "reset" });
      return loadSnapshot(announceResult);
    },
    [announce, loadSnapshot]
  );

  const selectedItem = useMemo(() => {
    const items =
      snapshot && ["ready", "stale"].includes(snapshot.reviewEnvelope.status)
        ? (snapshot.reviewEnvelope.data?.items ?? [])
        : [];
    return items.find(item => item.id === selectedItemId) ?? items[0] ?? null;
  }, [selectedItemId, snapshot]);

  const executeReviewAction = useCallback(
    async (action: ReviewAction) => {
      if (activeReviewOperationRef.current !== null || activeResumeOperationRef.current !== null) {
        announce("已有决定或任务请求正在核对；没有并发发送命令。");
        return;
      }
      const operationId = ++operationSequenceRef.current;
      activeReviewOperationRef.current = operationId;
      try {
        if (!dataSource) {
          dispatchReview({
            type: "dispatch_failed",
            errorCode: "governed_action_data_source_unavailable",
          });
          announce("审核决定未发送：数据源不可用。");
          return;
        }
        try {
          await dataSource.dispatchReviewAction(action);
          dispatchReview({ type: "dispatch_succeeded" });
        } catch (error) {
          dispatchReview({ type: "dispatch_failed", errorCode: errorCode(error) });
          announce("审核决定记录失败；任务继续保持暂停。");
          return;
        }

        const refreshed = await loadSnapshot(false);
        if (
          refreshed.reviewEnvelope.status !== "ready" &&
          refreshed.reviewEnvelope.status !== "empty"
        ) {
          dispatchReview({
            type: "refresh_failed",
            errorCode: `review_refresh_status_${refreshed.reviewEnvelope.status}`,
          });
          announce("决定已发送，但审核读模型未能完成核对；当前不展示成功结论。");
          return;
        }
        const item = refreshed.reviewEnvelope.data?.items.find(
          candidate => candidate.id === action.targetReviewItemId
        );
        if (!item) {
          dispatchReview({ type: "refresh_failed", errorCode: "review_refresh_target_missing" });
          announce("决定已发送，但刷新后找不到同一审核项；当前不展示成功结论。");
          return;
        }
        const refreshEvent = {
          type: "refresh_succeeded",
          item: {
            reviewItemId: item.id,
            status: item.status,
            materializationStatus: item.materializationStatus,
          },
        } as const;
        const verification = reviewDispatchReducer({ phase: "refreshing", action }, refreshEvent);
        dispatchReview(refreshEvent);
        if (verification.phase !== "resolved") {
          announce("决定已发送，但刷新后的同一审核项尚未确认该决定；任务仍暂停。");
        } else if (action.kind === "approve") {
          announce("权限决定已由刷新后的审核读模型确认；任务尚未继续。");
        } else if (action.kind === "reject") {
          announce("拒绝决定已由刷新后的审核读模型确认。");
        } else if (action.kind === "later") {
          announce("稍后处理已由刷新后的审核读模型确认；任务仍暂停。");
        } else {
          announce("决定已刷新；页面没有从命令回调推断后续结果。");
        }
      } finally {
        if (activeReviewOperationRef.current === operationId) {
          activeReviewOperationRef.current = null;
        }
      }
    },
    [announce, dataSource, loadSnapshot]
  );

  const requestReviewAction = useCallback(
    (action: ReviewAction) => {
      if (activeReviewOperationRef.current !== null || activeResumeOperationRef.current !== null) {
        announce("已有决定或任务请求正在核对；没有并发发送命令。");
        return;
      }
      const guardedAction = reviewEnvelopeAllowsDecisions(snapshot)
        ? action
        : {
            ...action,
            enabled: false,
            disabledReason: "审核读模型不是可用状态；请先重新读取。",
          };
      const next = reviewDispatchReducer(initialReviewDispatchState, {
        type: "request",
        action: guardedAction,
      });
      dispatchReview({ type: "request", action: guardedAction });
      if (next.phase === "blocked") {
        announce(`当前不能记录决定：${next.reason}`);
        return;
      }
      if (next.phase === "confirming") {
        announce("等待你确认这项决定；尚未发送任何命令。");
        return;
      }
      if (next.phase === "dispatching") {
        announce("正在记录决定；命令返回后仍需刷新核对。");
        void executeReviewAction(guardedAction);
      }
    },
    [announce, executeReviewAction, snapshot]
  );

  const confirmReviewAction = useCallback(() => {
    if (reviewState.phase !== "confirming") return;
    const action = reviewState.action;
    dispatchReview({ type: "confirm" });
    announce("正在记录决定；命令返回后仍需刷新核对。");
    void executeReviewAction(action);
  }, [announce, executeReviewAction, reviewState]);

  const cancelReviewConfirmation = useCallback(() => {
    dispatchReview({ type: "cancel_confirmation" });
    announce("已取消确认；没有发送审核决定。");
  }, [announce]);

  const executeResume = useCallback(
    async (control: TaskControl) => {
      if (activeReviewOperationRef.current !== null || activeResumeOperationRef.current !== null) {
        announce("已有决定或任务请求正在核对；没有并发发送命令。");
        return;
      }
      const operationId = ++operationSequenceRef.current;
      activeResumeOperationRef.current = operationId;
      try {
        if (!dataSource) {
          dispatchResume({
            type: "dispatch_failed",
            errorCode: "governed_action_data_source_unavailable",
          });
          announce("任务恢复请求未发送：数据源不可用。");
          return;
        }
        try {
          await dataSource.resumeTask(control);
          dispatchResume({ type: "dispatch_succeeded" });
        } catch (error) {
          dispatchResume({ type: "dispatch_failed", errorCode: errorCode(error) });
          announce("任务恢复请求失败；当前不会显示为运行或完成。");
          return;
        }

        const refreshed = await loadSnapshot(false);
        if (refreshed.tasksEnvelope.status !== "ready") {
          dispatchResume({
            type: "refresh_failed",
            errorCode: `task_refresh_status_${refreshed.tasksEnvelope.status}`,
          });
          announce("恢复请求已发送，但任务读模型未能完成核对；当前不展示运行结论。");
          return;
        }
        const task = findRefreshedTask(refreshed, control.targetTaskId);
        const refreshEvent = { type: "refresh_succeeded", task } as const;
        const verification = taskResumeReducer({ phase: "refreshing", control }, refreshEvent);
        dispatchResume(refreshEvent);
        if (verification.phase === "failed") {
          announce("恢复请求已发送，但刷新后的任务身份不一致；当前不展示运行结论。");
        } else if (verification.phase === "awaiting_projection" && !task) {
          announce("恢复请求已发送，但刷新后找不到同一任务；当前继续保持未知。");
        } else if (verification.phase === "awaiting_projection") {
          announce("恢复请求已发送，但刷新后的同一任务仍未确认继续；当前保持暂停。");
        } else if (verification.phase === "resolved" && task?.lifecycleStatus === "running") {
          announce("刷新后的同一任务已进入运行中；这还不是完成结论。");
        } else if (
          verification.phase === "resolved" &&
          task?.lifecycleStatus === "completed" &&
          task.terminalDeliveryStatus === "delivered" &&
          task.finalDeliveryEvidencePresent
        ) {
          announce("刷新后的任务状态与最终交付证据一致，任务已完成。");
        } else {
          announce("任务状态已经刷新；页面不会把恢复请求解释成完成。");
        }
      } finally {
        if (activeResumeOperationRef.current === operationId) {
          activeResumeOperationRef.current = null;
        }
      }
    },
    [announce, dataSource, loadSnapshot]
  );

  const requestResume = useCallback(
    (control: TaskControl, expectedTaskId: string) => {
      if (activeReviewOperationRef.current !== null || activeResumeOperationRef.current !== null) {
        announce("已有决定或任务请求正在核对；没有并发发送命令。");
        return;
      }
      const guardedControl = workspaceEnvelopeAllowsControls(snapshot)
        ? control
        : {
            ...control,
            enabled: false,
            disabledReason: "工作区读模型不是可用状态；请先重新读取。",
          };
      const next = taskResumeReducer(initialTaskResumeState, {
        type: "request",
        control: guardedControl,
        expectedTaskId,
      });
      dispatchResume({ type: "request", control: guardedControl, expectedTaskId });
      if (next.phase === "blocked") {
        announce(`当前不能继续任务：${next.reason}`);
        return;
      }
      if (next.phase === "confirming") {
        announce("等待你确认继续任务；尚未发送恢复请求。");
        return;
      }
      if (next.phase === "dispatching") {
        announce("正在发送任务恢复请求；命令返回后仍需刷新核对。");
        void executeResume(guardedControl);
      }
    },
    [announce, executeResume, snapshot]
  );

  const confirmResume = useCallback(() => {
    if (resumeState.phase !== "confirming") return;
    const control = resumeState.control;
    dispatchResume({ type: "confirm" });
    announce("正在发送任务恢复请求；命令返回后仍需刷新核对。");
    void executeResume(control);
  }, [announce, executeResume, resumeState]);

  const cancelResumeConfirmation = useCallback(() => {
    dispatchResume({ type: "cancel_confirmation" });
    announce("已取消继续任务；没有发送恢复请求。");
  }, [announce]);

  const selectReviewItem = useCallback(
    (itemOrId: ReviewItem | string) => {
      const nextId = typeof itemOrId === "string" ? itemOrId : itemOrId.id;
      if (nextId !== selectedItemId && activeReviewOperationRef.current === null) {
        dispatchReview({ type: "reset" });
      }
      setSelectedItemId(nextId);
    },
    [selectedItemId]
  );

  return {
    snapshot,
    selectedItem,
    refreshing,
    reviewState,
    resumeState,
    load,
    selectReviewItem,
    requestReviewAction,
    confirmReviewAction,
    cancelReviewConfirmation,
    requestResume,
    confirmResume,
    cancelResumeConfirmation,
  };
}
