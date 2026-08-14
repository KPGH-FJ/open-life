import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import type { ReviewAction, ReviewItem, TaskControl, TaskViewModelItem } from "@/tauri";
import { journeyErrorCode as errorCode } from "@/ui/journeys/journeyError";
import {
  initialReviewDispatchState,
  reviewDispatchReducer,
  type ReviewDispatchState,
} from "@/contracts/reviewDispatchContract";
import {
  initialTaskControlDispatchState,
  taskControlDispatchReducer,
  type TaskControlDispatchState,
} from "./taskControlContract";
import {
  buildGovernedActionErrorSnapshot,
  type GovernedActionDataSource,
  type GovernedActionSnapshot,
} from "./governedActionDataSource";

type Announce = (message: string) => void;

function reviewEnvelopeAllowsDecisions(snapshot: GovernedActionSnapshot | null): boolean {
  return snapshot?.reviewEnvelope.status === "ready";
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
  taskControlState: TaskControlDispatchState;
  load: (
    announceResult?: boolean,
    conversationId?: string | null
  ) => Promise<GovernedActionSnapshot>;
  selectReviewItem: (itemOrId: ReviewItem | string) => void;
  requestReviewAction: (action: ReviewAction) => void;
  confirmReviewAction: () => void;
  cancelReviewConfirmation: () => void;
  editLifeModelLearning: (statement: string) => Promise<boolean>;
  requestTaskControl: (control: TaskControl, expectedTaskId: string) => void;
  confirmTaskControl: () => void;
  cancelTaskControlConfirmation: () => void;
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
  const [taskControlState, dispatchTaskControlState] = useReducer(
    taskControlDispatchReducer,
    initialTaskControlDispatchState
  );
  const snapshotRef = useRef<GovernedActionSnapshot | null>(null);
  const conversationIdRef = useRef<string | null>(null);
  const requestRef = useRef(0);
  const operationSequenceRef = useRef(0);
  const activeReviewOperationRef = useRef<number | null>(null);
  const activeTaskControlOperationRef = useRef<number | null>(null);

  useEffect(() => {
    requestRef.current += 1;
    activeReviewOperationRef.current = null;
    activeTaskControlOperationRef.current = null;
    conversationIdRef.current = null;
    snapshotRef.current = null;
    setSnapshot(null);
    setSelectedItemId(null);
    setRefreshing(false);
    dispatchReview({ type: "reset" });
    dispatchTaskControlState({ type: "reset" });
    return () => {
      requestRef.current += 1;
    };
  }, [dataSource]);

  const loadSnapshot = useCallback(
    async (
      announceResult = true,
      conversationId: string | null = conversationIdRef.current
    ): Promise<GovernedActionSnapshot> => {
      const requestId = ++requestRef.current;
      conversationIdRef.current = conversationId;
      setRefreshing(true);
      let next: GovernedActionSnapshot;
      try {
        next = dataSource
          ? await dataSource.load(conversationId)
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
    async (
      announceResult = true,
      conversationId: string | null = conversationIdRef.current
    ): Promise<GovernedActionSnapshot> => {
      if (
        activeReviewOperationRef.current !== null ||
        activeTaskControlOperationRef.current !== null
      ) {
        announce("已有决定或任务请求正在核对；本次没有并发刷新。");
        return (
          snapshotRef.current ??
          buildGovernedActionErrorSnapshot("governed_action_refresh_blocked_without_snapshot")
        );
      }
      dispatchReview({ type: "reset" });
      dispatchTaskControlState({ type: "reset" });
      return loadSnapshot(announceResult, conversationId);
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

  const editLifeModelLearning = useCallback(
    async (statement: string): Promise<boolean> => {
      const item = selectedItem;
      if (
        !dataSource ||
        !item?.decisionContext.lifeModelLearning ||
        !reviewEnvelopeAllowsDecisions(snapshotRef.current) ||
        activeReviewOperationRef.current !== null ||
        activeTaskControlOperationRef.current !== null
      ) {
        announce("当前审核项不能使用 LifeModel 结构化编辑器。");
        return false;
      }
      const operationId = ++operationSequenceRef.current;
      activeReviewOperationRef.current = operationId;
      try {
        await dataSource.editLifeModelLearningProposal(item.source.proposalId, statement);
        const refreshed = await loadSnapshot(false);
        const revised = refreshed.reviewEnvelope.data?.items.find(
          candidate => candidate.id === item.id
        );
        if (
          refreshed.reviewEnvelope.status !== "ready" ||
          revised?.status !== "edited" ||
          revised.decisionContext.lifeModelLearning?.proposedStatement !== statement.trim()
        ) {
          announce("修改已发送，但刷新后的审核项无法证明新内容，当前不报告成功。");
          return false;
        }
        setSelectedItemId(revised.id);
        announce("审核内容已按 LifeModel schema 更新；尚未写入规范版本。");
        return true;
      } catch (error) {
        announce(`LifeModel 审核内容未修改：${errorCode(error)}`);
        return false;
      } finally {
        if (activeReviewOperationRef.current === operationId) {
          activeReviewOperationRef.current = null;
        }
      }
    },
    [announce, dataSource, loadSnapshot, selectedItem]
  );

  const executeReviewAction = useCallback(
    async (action: ReviewAction) => {
      if (
        activeReviewOperationRef.current !== null ||
        activeTaskControlOperationRef.current !== null
      ) {
        announce("已有决定或任务请求正在核对；没有并发发送命令。");
        return;
      }
      const operationId = ++operationSequenceRef.current;
      activeReviewOperationRef.current = operationId;
      const dispatchedItem = snapshotRef.current?.reviewEnvelope.data?.items.find(
        item => item.id === action.targetReviewItemId
      );
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
        } else if (action.kind === "approve" && dispatchedItem?.type === "tool_permission") {
          announce("权限决定已由刷新后的审核读模型确认；任务尚未继续。");
        } else if (action.kind === "approve") {
          announce("批准决定已由刷新后的审核读模型确认；应用结果仍需独立刷新证明。");
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
      if (
        activeReviewOperationRef.current !== null ||
        activeTaskControlOperationRef.current !== null
      ) {
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

  const executeTaskControl = useCallback(
    async (control: TaskControl) => {
      if (
        activeReviewOperationRef.current !== null ||
        activeTaskControlOperationRef.current !== null
      ) {
        announce("已有决定或任务请求正在核对；没有并发发送命令。");
        return;
      }
      const operationId = ++operationSequenceRef.current;
      activeTaskControlOperationRef.current = operationId;
      try {
        if (!dataSource) {
          dispatchTaskControlState({
            type: "dispatch_failed",
            errorCode: "governed_action_data_source_unavailable",
          });
          announce("任务请求未发送：数据源不可用。");
          return;
        }
        try {
          await dataSource.dispatchTaskControl(control);
          dispatchTaskControlState({ type: "dispatch_succeeded" });
        } catch (error) {
          dispatchTaskControlState({ type: "dispatch_failed", errorCode: errorCode(error) });
          announce("任务请求失败；当前不会显示状态已经改变。");
          return;
        }

        const refreshed = await loadSnapshot(false);
        if (refreshed.tasksEnvelope.status !== "ready") {
          dispatchTaskControlState({
            type: "refresh_failed",
            errorCode: `task_refresh_status_${refreshed.tasksEnvelope.status}`,
          });
          announce("任务请求已发送，但任务读模型未能完成核对；当前不展示成功结论。");
          return;
        }
        const task = findRefreshedTask(refreshed, control.targetTaskId);
        const refreshEvent = { type: "refresh_succeeded", task } as const;
        const verification = taskControlDispatchReducer(
          { phase: "refreshing", control },
          refreshEvent
        );
        dispatchTaskControlState(refreshEvent);
        if (verification.phase === "failed") {
          announce("任务请求已发送，但刷新后的任务身份不一致；当前不展示成功结论。");
        } else if (verification.phase === "awaiting_projection" && !task) {
          announce("任务请求已发送，但刷新后找不到同一任务；当前保持未知。");
        } else if (verification.phase === "awaiting_projection") {
          announce("任务请求已发送，但刷新后的同一任务尚未确认该变化。");
        } else if (verification.phase === "resolved" && control.kind === "cancel") {
          announce("刷新后的同一任务已确认取消。");
        } else if (verification.phase === "resolved" && control.kind === "refresh_context") {
          announce("任务上下文已经重新读取；任务结果没有因此被解释成完成。");
        } else if (verification.phase === "resolved" && control.kind === "retry") {
          announce("刷新后的同一任务已离开失败状态；这还不是完成结论。");
        } else if (verification.phase === "resolved") {
          announce("刷新后的同一任务已确认继续；这还不是完成结论。");
        }
      } finally {
        if (activeTaskControlOperationRef.current === operationId) {
          activeTaskControlOperationRef.current = null;
        }
      }
    },
    [announce, dataSource, loadSnapshot]
  );

  const requestTaskControl = useCallback(
    (control: TaskControl, expectedTaskId: string) => {
      if (
        activeReviewOperationRef.current !== null ||
        activeTaskControlOperationRef.current !== null
      ) {
        announce("已有决定或任务请求正在核对；没有并发发送命令。");
        return;
      }
      const guardedControl =
        snapshot?.tasksEnvelope.status === "ready"
          ? control
          : {
              ...control,
              enabled: false,
              disabledReason: "任务读模型不是可用状态；请先重新读取。",
            };
      const next = taskControlDispatchReducer(initialTaskControlDispatchState, {
        type: "request",
        control: guardedControl,
        expectedTaskId,
      });
      dispatchTaskControlState({
        type: "request",
        control: guardedControl,
        expectedTaskId,
      });
      if (next.phase === "blocked") {
        announce(`当前不能执行任务动作：${next.reason}`);
      } else if (next.phase === "confirming") {
        announce("等待你确认这项任务动作；尚未发送命令。");
      } else if (next.phase === "dispatching") {
        announce("正在发送任务请求；命令返回后仍需刷新核对。");
        void executeTaskControl(guardedControl);
      }
    },
    [announce, executeTaskControl, snapshot]
  );

  const confirmTaskControl = useCallback(() => {
    if (taskControlState.phase !== "confirming") return;
    const control = taskControlState.control;
    dispatchTaskControlState({ type: "confirm" });
    announce("正在发送任务请求；命令返回后仍需刷新核对。");
    void executeTaskControl(control);
  }, [announce, executeTaskControl, taskControlState]);

  const cancelTaskControlConfirmation = useCallback(() => {
    dispatchTaskControlState({ type: "cancel_confirmation" });
    announce("已取消确认；没有发送任务命令。");
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
    taskControlState,
    load,
    selectReviewItem,
    requestReviewAction,
    confirmReviewAction,
    cancelReviewConfirmation,
    editLifeModelLearning,
    requestTaskControl,
    confirmTaskControl,
    cancelTaskControlConfirmation,
  };
}
