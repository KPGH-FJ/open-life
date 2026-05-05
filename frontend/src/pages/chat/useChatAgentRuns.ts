import { useCallback, useRef, useState } from "react";
import { listAgentRunsForSession, getAgentRun, type AgentRun } from "../../tauri";

export function useChatAgentRuns() {
  const [currentRun, setCurrentRun] = useState<AgentRun | null>(null);
  const [currentRunId, setCurrentRunId] = useState<string | null>(null);
  const sessionIdRef = useRef<string>("");

  const setSessionId = useCallback((id: string) => {
    sessionIdRef.current = id;
  }, []);

  const refreshAgentRuns = useCallback(async (sessionId?: string) => {
    const sid = sessionId ?? sessionIdRef.current;
    try {
      const runs = await listAgentRunsForSession(sid, 10);
      if (sessionIdRef.current === sid) {
        setCurrentRun(runs[0] ?? null);
      }
    } catch {
      // silently ignore
    }
  }, []);

  const loadAgentRunForSession = useCallback(
    async (runId: string | undefined, sessionId: string) => {
      if (!runId) return;
      try {
        const run = await getAgentRun(runId);
        if (sessionIdRef.current === sessionId) {
          setCurrentRun(run);
        }
      } catch {
        // silently ignore
      }
    },
    []
  );

  return {
    currentRun,
    setCurrentRun,
    currentRunId,
    setCurrentRunId,
    setSessionId,
    refreshAgentRuns,
    loadAgentRunForSession,
  };
}
