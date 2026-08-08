import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { DraftLegacyLifeModelMigrationRequest, ReviewItem } from "@/tauri";
import {
  buildDurableTruthErrorSnapshot,
  type DurableTruthDataSource,
  type DurableTruthSnapshot,
} from "./durableTruthDataSource";
import { durableReviewItems } from "./durableTruthPresentation";
import { journeyErrorCode as errorText } from "@/ui/journeys/journeyError";

type Announce = (message: string) => void;

function preferredDurableItem(items: ReviewItem[]): ReviewItem | null {
  return (
    items.find(item => item.type !== "memory_write" && item.type !== "memory_archive") ??
    items[0] ??
    null
  );
}

export function useDurableTruthJourney(
  dataSource: DurableTruthDataSource | undefined,
  announce: Announce
) {
  const [snapshot, setSnapshot] = useState<DurableTruthSnapshot | null>(null);
  const [selectedItemId, setSelectedItemId] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [memoryAction, setMemoryAction] = useState<{
    memoryId: string;
    action: "correct" | "stop_recall" | "archive" | "restore" | "rollback" | "erase";
    error?: string;
  } | null>(null);
  const [migrationAction, setMigrationAction] = useState<{
    status: "submitting" | "review_required" | "failed";
    proposalId?: string;
    error?: string;
  } | null>(null);
  const requestRef = useRef(0);

  useEffect(() => {
    requestRef.current += 1;
    setSnapshot(null);
    setSelectedItemId(null);
    setRefreshing(false);
    setMemoryAction(null);
    setMigrationAction(null);
    return () => {
      requestRef.current += 1;
    };
  }, [dataSource]);

  const load = useCallback(
    async (announceResult = true): Promise<DurableTruthSnapshot> => {
      const requestId = ++requestRef.current;
      setRefreshing(true);
      let next: DurableTruthSnapshot;
      try {
        next = dataSource
          ? await dataSource.loadDurableTruth()
          : buildDurableTruthErrorSnapshot("durable_truth_data_source_unavailable");
      } catch (error) {
        next = buildDurableTruthErrorSnapshot(error);
      }
      if (requestId === requestRef.current) {
        setSnapshot(next);
        const items = durableReviewItems(next);
        setSelectedItemId(current =>
          current && items.some(item => item.id === current)
            ? current
            : (preferredDurableItem(items)?.id ?? null)
        );
        setRefreshing(false);
        if (announceResult) {
          const failed = [
            next.lifeModelEnvelope.status,
            next.memoryEnvelope.status,
            next.reviewEnvelope.status,
          ].some(status => status === "error");
          announce(
            failed
              ? "长期状态读取不完整；当前不确认决定或应用结果。"
              : "LifeModel、Memory 与审核状态已从后端读模型重新核对。"
          );
        }
      }
      return next;
    },
    [announce, dataSource]
  );

  const selectedItem = useMemo(() => {
    const items = durableReviewItems(snapshot);
    return items.find(item => item.id === selectedItemId) ?? preferredDurableItem(items);
  }, [selectedItemId, snapshot]);

  const selectItem = useCallback((item: ReviewItem) => setSelectedItemId(item.id), []);

  const runMemoryAction = useCallback(
    async (
      memoryId: string,
      action: "correct" | "stop_recall" | "archive" | "restore" | "rollback" | "erase",
      operation: (source: DurableTruthDataSource) => Promise<void>,
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
        "Memory 纠正已进入 Review；旧记忆仍保持当前状态。"
      ),
    [runMemoryAction]
  );
  const archiveMemory = useCallback(
    (memoryId: string) =>
      runMemoryAction(
        memoryId,
        "archive",
        source => source.archiveMemory(memoryId),
        "归档已进入 Review；确认应用前仍保持当前状态。"
      ),
    [runMemoryAction]
  );
  const stopRecall = useCallback(
    (memoryId: string) =>
      runMemoryAction(
        memoryId,
        "stop_recall",
        source => source.stopRecall(memoryId),
        "停止召回已进入 Review；确认应用前仍会正常召回。"
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
  const draftLegacyMigration = useCallback(
    async (request: DraftLegacyLifeModelMigrationRequest): Promise<boolean> => {
      if (!dataSource || migrationAction?.status === "submitting") return false;
      setMigrationAction({ status: "submitting" });
      let proposalId: string;
      try {
        proposalId = await dataSource.draftLegacyLifeModelMigration(request);
      } catch (error) {
        const reason = errorText(error);
        setMigrationAction({ status: "failed", error: reason });
        announce(`迁移建议未创建：${reason}`);
        return false;
      }

      const refreshed = await load(false);
      setMigrationAction({ status: "review_required", proposalId });
      if (refreshed.reviewEnvelope.status === "error") {
        announce(
          "迁移建议已经创建，但 Review 状态刷新未验证；旧 YAML 仍是当前来源，请在 Review 中重新核对。"
        );
      } else {
        announce("迁移选择已进入 Review；旧 YAML 仍是当前来源，尚未发生切换。");
      }
      return true;
    },
    [announce, dataSource, load, migrationAction?.status]
  );

  return {
    snapshot,
    selectedItem,
    refreshing,
    memoryAction,
    migrationAction,
    load,
    selectItem,
    correctMemory,
    archiveMemory,
    stopRecall,
    restoreMemory,
    rollbackMemory,
    privacyEraseMemory,
    draftLegacyMigration,
  };
}
