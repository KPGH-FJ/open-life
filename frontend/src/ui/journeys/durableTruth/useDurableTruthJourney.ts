import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ReviewItem } from "@/tauri";
import {
  buildDurableTruthErrorSnapshot,
  type DurableTruthDataSource,
  type DurableTruthSnapshot,
} from "./durableTruthDataSource";
import { durableReviewItems } from "./durableTruthPresentation";

type Announce = (message: string) => void;

export function useDurableTruthJourney(
  dataSource: DurableTruthDataSource | undefined,
  announce: Announce
) {
  const [snapshot, setSnapshot] = useState<DurableTruthSnapshot | null>(null);
  const [selectedItemId, setSelectedItemId] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const requestRef = useRef(0);

  useEffect(() => {
    requestRef.current += 1;
    setSnapshot(null);
    setSelectedItemId(null);
    setRefreshing(false);
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
          current && items.some(item => item.id === current) ? current : (items[0]?.id ?? null)
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
    return items.find(item => item.id === selectedItemId) ?? items[0] ?? null;
  }, [selectedItemId, snapshot]);

  const selectItem = useCallback((item: ReviewItem) => setSelectedItemId(item.id), []);

  return { snapshot, selectedItem, refreshing, load, selectItem };
}
