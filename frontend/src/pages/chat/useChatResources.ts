import { useCallback, useRef, useState } from "react";
import {
  cancelResourceImport,
  detachResourceFromTurn,
  getResourceImportStatus,
  pickAndImportResources,
} from "../../tauri";
import type { ImportedResourceReceipt, ResourceImportReceipt } from "../../tauri";

export interface ResourceDraft {
  turnOperationId: string;
  resources: ImportedResourceReceipt[];
}

interface ActiveResourceImport {
  operationId: string;
  sessionId: string;
}

interface UseChatResourcesInput {
  sessionId: string;
  interactionBlocked: boolean;
}

function importFailureMessage(error: unknown): string {
  const detail = error instanceof Error ? error.message : String(error);
  if (detail.includes("file_count_exceeded")) {
    return "一次最多选择 5 个文件，请减少文件数量后重试。";
  }
  if (detail.includes("file_bytes_exceeded") || detail.includes("import_bytes_exceeded")) {
    return "所选文件超过本轮允许的大小（单个 20 MB、合计 50 MB）。";
  }
  if (detail.includes("unsupported") || detail.includes("mime") || detail.includes("format")) {
    return "文件格式或实际内容不受支持，请换用 PDF、DOCX、TXT、Markdown、CSV 或 XLSX。";
  }
  if (detail.includes("resource_runtime_unavailable")) {
    return "文件能力当前不可用，OpenLife 没有降级到临时存储。请重启后再试。";
  }
  return "文件导入结果无法确认；OpenLife 没有把它显示为已加入。请先核对状态，再决定是否重试。";
}

export function useChatResources({ sessionId, interactionBlocked }: UseChatResourcesInput) {
  const [drafts, setDrafts] = useState<Record<string, ResourceDraft>>({});
  const [importBusy, setImportBusy] = useState(false);
  const [activeImport, setActiveImport] = useState<ActiveResourceImport | null>(null);
  const [errors, setErrors] = useState<Record<string, string | null>>({});
  const [notices, setNotices] = useState<Record<string, string | null>>({});
  const [removingResourceIds, setRemovingResourceIds] = useState<string[]>([]);
  const pendingDetachOperationIdsRef = useRef<Map<string, string>>(new Map());

  const acceptImportReceipt = useCallback(
    (
      targetSessionId: string,
      importOperationId: string,
      turnOperationId: string,
      receipt: ResourceImportReceipt | null
    ): boolean => {
      if (
        !receipt ||
        receipt.operationId !== importOperationId ||
        receipt.messageId !== turnOperationId ||
        receipt.resources.length === 0 ||
        (drafts[targetSessionId] && drafts[targetSessionId].turnOperationId !== turnOperationId)
      ) {
        return false;
      }
      setDrafts(previous => {
        const existing = previous[targetSessionId];
        if (existing && existing.turnOperationId !== turnOperationId) return previous;
        const merged = new Map(
          (existing?.resources ?? []).map(resource => [resource.resourceId, resource])
        );
        for (const resource of receipt.resources) merged.set(resource.resourceId, resource);
        return {
          ...previous,
          [targetSessionId]: { turnOperationId, resources: [...merged.values()] },
        };
      });
      return true;
    },
    [drafts]
  );

  const attachResources = useCallback(async () => {
    if (importBusy || interactionBlocked || !sessionId) return;
    const targetSessionId = sessionId;
    const turnOperationId = drafts[targetSessionId]?.turnOperationId ?? crypto.randomUUID();
    const importOperationId = crypto.randomUUID();
    setImportBusy(true);
    setActiveImport({ operationId: importOperationId, sessionId: targetSessionId });
    setErrors(previous => ({ ...previous, [targetSessionId]: null }));
    setNotices(previous => ({ ...previous, [targetSessionId]: null }));
    try {
      const result = await pickAndImportResources(importOperationId, turnOperationId);
      if (result.cancelled) {
        setNotices(previous => ({
          ...previous,
          [targetSessionId]: "未选择文件，本轮附件没有变化。",
        }));
        return;
      }
      const receipt = result.receipt;
      if (
        !receipt ||
        !acceptImportReceipt(targetSessionId, importOperationId, turnOperationId, receipt)
      ) {
        throw new Error("resource_import_receipt_mismatch");
      }
      setNotices(previous => ({
        ...previous,
        [targetSessionId]: `已加入 ${receipt.resources.length} 个文件；发送时才会把选中的片段交给模型。`,
      }));
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      if (detail.includes("resource_import_cancelled")) {
        setNotices(previous => ({
          ...previous,
          [targetSessionId]: "导入已停止，未提交新的附件。",
        }));
      } else {
        let reconciled = false;
        try {
          const status = await getResourceImportStatus(importOperationId);
          if (
            status.status === "committed" &&
            acceptImportReceipt(targetSessionId, importOperationId, turnOperationId, status.receipt)
          ) {
            reconciled = true;
            setNotices(previous => ({
              ...previous,
              [targetSessionId]: "导入响应曾中断，但 canonical receipt 已确认文件提交成功。",
            }));
          } else if (status.status === "not_found") {
            reconciled = true;
            const friendlyMessage = importFailureMessage(error);
            setErrors(previous => ({
              ...previous,
              [targetSessionId]: friendlyMessage.includes("结果无法确认")
                ? "文件导入失败，canonical store 确认没有提交该操作。"
                : friendlyMessage,
            }));
          }
        } catch {
          // The original operation remains unknown. Do not display it as
          // attached and do not create a new operation implicitly.
        }
        if (!reconciled) {
          setErrors(previous => ({
            ...previous,
            [targetSessionId]: importFailureMessage(error),
          }));
        }
      }
    } finally {
      setImportBusy(false);
      setActiveImport(current => (current?.operationId === importOperationId ? null : current));
    }
  }, [acceptImportReceipt, drafts, importBusy, interactionBlocked, sessionId]);

  const cancelImport = useCallback(async () => {
    if (!activeImport) return;
    const { operationId, sessionId: targetSessionId } = activeImport;
    try {
      const cancellationRequested = await cancelResourceImport(operationId);
      setNotices(previous => ({
        ...previous,
        [targetSessionId]: cancellationRequested
          ? "已请求停止导入；如果系统文件窗口仍打开，请关闭该窗口。"
          : "导入已经结束，没有仍在运行的导入任务。",
      }));
    } catch {
      setErrors(previous => ({
        ...previous,
        [targetSessionId]: "无法确认停止请求；在导入返回明确结果前不会把文件显示为已加入。",
      }));
    }
  }, [activeImport]);

  const removeResource = useCallback(
    async (resourceId: string) => {
      const targetSessionId = sessionId;
      const draft = drafts[targetSessionId];
      if (
        !draft ||
        !draft.resources.some(resource => resource.resourceId === resourceId) ||
        removingResourceIds.includes(resourceId) ||
        importBusy ||
        interactionBlocked
      ) {
        return;
      }
      setRemovingResourceIds(previous => [...previous, resourceId]);
      setErrors(previous => ({ ...previous, [targetSessionId]: null }));
      const detachKey = `${draft.turnOperationId}\u0000${resourceId}`;
      const detachOperationId =
        pendingDetachOperationIdsRef.current.get(detachKey) ?? crypto.randomUUID();
      pendingDetachOperationIdsRef.current.set(detachKey, detachOperationId);
      try {
        const receipt = await detachResourceFromTurn(
          detachOperationId,
          draft.turnOperationId,
          resourceId
        );
        if (
          receipt.operationId !== detachOperationId ||
          receipt.messageId !== draft.turnOperationId ||
          receipt.resourceId !== resourceId ||
          !receipt.bindingRemoved
        ) {
          throw new Error("resource_detach_receipt_mismatch");
        }
        pendingDetachOperationIdsRef.current.delete(detachKey);
        setDrafts(previous => {
          const current = previous[targetSessionId];
          if (!current || current.turnOperationId !== draft.turnOperationId) return previous;
          const resources = current.resources.filter(
            resource => resource.resourceId !== resourceId
          );
          if (resources.length === 0) {
            const { [targetSessionId]: _removed, ...remaining } = previous;
            return remaining;
          }
          return { ...previous, [targetSessionId]: { ...current, resources } };
        });
        setNotices(previous => ({
          ...previous,
          [targetSessionId]: receipt.resourceDeleted
            ? "附件已从本轮移除；OpenLife 中不再有其他引用，文件副本也已删除。"
            : "附件已从本轮移除；其他仍在使用它的轮次不受影响。",
        }));
      } catch {
        setErrors(previous => ({
          ...previous,
          [targetSessionId]: "附件移除结果无法确认，界面已保留该附件。请稍后用同一轮附件状态重试。",
        }));
      } finally {
        setRemovingResourceIds(previous => previous.filter(id => id !== resourceId));
      }
    },
    [drafts, importBusy, interactionBlocked, removingResourceIds, sessionId]
  );

  const completeTurn = useCallback((targetSessionId: string, turnOperationId: string) => {
    setDrafts(previous => {
      if (previous[targetSessionId]?.turnOperationId !== turnOperationId) return previous;
      const { [targetSessionId]: _completed, ...remaining } = previous;
      return remaining;
    });
    setErrors(previous => ({ ...previous, [targetSessionId]: null }));
    setNotices(previous => ({ ...previous, [targetSessionId]: null }));
  }, []);

  const currentDraft = drafts[sessionId];
  return {
    currentDraft,
    currentResources: currentDraft?.resources ?? [],
    importBusy,
    currentError: errors[sessionId] ?? null,
    currentNotice: notices[sessionId] ?? null,
    removingResourceIds,
    attachResources,
    cancelImport,
    removeResource,
    completeTurn,
  };
}
