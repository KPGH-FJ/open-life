import { useEffect, useState } from "react";
import {
  getLifeModel,
  getSystemDiagnostics,
  getSchedulerConfig,
  setSchedulerConfig,
  getPendingProposals,
  type SystemDiagnostics,
  type AgentProposal,
} from "../../tauri";
import type { LifeModel } from "../../types";
import { logError } from "../../utils/logger";

export function useChatContext() {
  const [diagnostics, setDiagnostics] = useState<SystemDiagnostics | null>(null);
  const [model, setModel] = useState<LifeModel | null>(null);
  const [preferLocal, setPreferLocal] = useState<boolean>(true);
  const [pendingProposals, setPendingProposals] = useState<AgentProposal[]>([]);

  // Initial load of diagnostics + scheduler config
  useEffect(() => {
    (async () => {
      try {
        const [diag, cfg] = await Promise.all([getSystemDiagnostics(), getSchedulerConfig()]);
        setDiagnostics(diag);
        setPreferLocal(cfg.preferLocal);
      } catch {
        // silently ignore
      }
    })();
  }, []);

  // Initial load of life model + proposals
  useEffect(() => {
    getLifeModel()
      .then(setModel)
      .catch(() => {});
    refreshPendingProposals();
  }, []);

  // Refresh on window focus
  useEffect(() => {
    const refresh = () => {
      getLifeModel()
        .then(setModel)
        .catch(() => {});
      getSystemDiagnostics()
        .then(setDiagnostics)
        .catch(() => {});
    };
    window.addEventListener("focus", refresh);
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "visible") refresh();
    });
    return () => {
      window.removeEventListener("focus", refresh);
      document.removeEventListener("visibilitychange", refresh);
    };
  }, []);

  function refreshPendingProposals() {
    getPendingProposals(10)
      .then(setPendingProposals)
      .catch(() => setPendingProposals([]));
  }

  const togglePreferLocal = async () => {
    const next = !preferLocal;
    setPreferLocal(next);
    try {
      const cfg = await getSchedulerConfig();
      await setSchedulerConfig(cfg.localModel, next);
      getSystemDiagnostics()
        .then(setDiagnostics)
        .catch(() => {});
    } catch (e) {
      logError(e);
    }
  };

  return {
    diagnostics,
    setDiagnostics,
    model,
    setModel,
    preferLocal,
    togglePreferLocal,
    pendingProposals,
    refreshPendingProposals,
  };
}
