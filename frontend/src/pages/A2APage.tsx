import { useEffect, useState } from "react";
import { Network, Send, RefreshCw, Shield, Sparkles, Play, Square } from "lucide-react";
import {
  a2aDiscoverAgent,
  a2aSendTask,
  a2aLocalAgentCard,
  a2aHandleTask,
  a2aBridgeLocal,
  a2aRestartSidecar,
  a2aStopSidecar,
} from "../tauri";
import ErrorBanner from "../components/ErrorBanner";
import ConfirmDangerDialog from "../components/ConfirmDangerDialog";

type PendingA2AAction = "discover" | "send" | "restart" | "stop";

function isLoopbackOrPrivateUrl(value: string): boolean {
  try {
    const url = new URL(value);
    const host = url.hostname.toLowerCase();
    if (host === "localhost" || host === "::1" || host.startsWith("127.")) return true;
    if (host.startsWith("10.") || host.startsWith("192.168.")) return true;
    const parts = host.split(".").map(part => Number(part));
    return parts.length === 4 && parts[0] === 172 && parts[1] >= 16 && parts[1] <= 31;
  } catch {
    return false;
  }
}

function StructuredJsonResult({ value }: { value: string }) {
  let parsed: any = null;
  try {
    parsed = JSON.parse(value);
  } catch {
    parsed = null;
  }

  if (!parsed) {
    return <pre className="bg-gray-50 border rounded-lg p-4 text-xs overflow-auto max-h-64">{value}</pre>;
  }

  const status = parsed.status?.state ?? parsed.status ?? parsed.result?.status ?? "unknown";
  const skill =
    parsed.metadata?.skill ??
    parsed.message?.metadata?.skill ??
    parsed.result?.metadata?.skill ??
    parsed.skill ??
    "未声明";
  const textPart =
    parsed.message?.parts?.find?.((part: any) => part.type === "text")?.text ??
    parsed.result?.message?.parts?.find?.((part: any) => part.type === "text")?.text ??
    parsed.artifacts?.[0]?.parts?.find?.((part: any) => part.type === "text")?.text ??
    parsed.text ??
    "";

  return (
    <div className="rounded-lg border bg-gray-50 p-4 text-sm text-gray-700">
      <div className="grid gap-2 sm:grid-cols-3">
        <div>
          <div className="text-[11px] font-medium uppercase text-gray-400">Status</div>
          <div className="font-semibold text-gray-900">{String(status)}</div>
        </div>
        <div>
          <div className="text-[11px] font-medium uppercase text-gray-400">Skill</div>
          <div className="font-semibold text-gray-900">{String(skill)}</div>
        </div>
        <div>
          <div className="text-[11px] font-medium uppercase text-gray-400">ID</div>
          <div className="truncate font-mono text-xs text-gray-600">{parsed.id ?? "n/a"}</div>
        </div>
      </div>
      {textPart && <div className="mt-3 whitespace-pre-wrap text-sm">{String(textPart)}</div>}
      <details className="mt-3">
        <summary className="cursor-pointer text-xs font-medium text-gray-500">Raw JSON</summary>
        <pre className="mt-2 max-h-64 overflow-auto rounded bg-white p-3 text-xs">
          {JSON.stringify(parsed, null, 2)}
        </pre>
      </details>
    </div>
  );
}

export default function A2APage() {
  const [agentUrl, setAgentUrl] = useState("http://127.0.0.1:8080");
  const [agentCard, setAgentCard] = useState<any | null>(null);
  const [loadingDiscover, setLoadingDiscover] = useState(false);

  const [taskText, setTaskText] = useState("");
  const [taskSkill, setTaskSkill] = useState("");
  const [taskResult, setTaskResult] = useState<string | null>(null);
  const [loadingTask, setLoadingTask] = useState(false);

  const [localCard, setLocalCard] = useState<any | null>(null);
  const [localResult, setLocalResult] = useState<string | null>(null);
  const [loadingLocal, setLoadingLocal] = useState(false);
  const [localServiceInput, setLocalServiceInput] = useState("");
  const [activeLocalSkill, setActiveLocalSkill] = useState<string | null>(null);
  const [bridgeMethod, setBridgeMethod] = useState("a2a.send");
  const [bridgeInput, setBridgeInput] = useState("");
  const [bridgeResult, setBridgeResult] = useState<string | null>(null);
  const [bridgeLoading, setBridgeLoading] = useState(false);
  const [sidecarMsg, setSidecarMsg] = useState<string | null>(null);
  const [pageError, setPageError] = useState<string>("");
  const [pendingAction, setPendingAction] = useState<PendingA2AAction | null>(null);

  useEffect(() => {
    a2aLocalAgentCard()
      .then(setLocalCard)
      .catch(() => {});
  }, []);

  const refreshLocalCard = async () => {
    const card = await a2aLocalAgentCard();
    setLocalCard(card);
  };

  const runDiscover = async () => {
    setLoadingDiscover(true);
    setPageError("");
    try {
      const card = await a2aDiscoverAgent(agentUrl);
      setAgentCard(card);
    } catch (e) {
      setAgentCard(null);
      setPageError("发现失败: " + String(e));
    } finally {
      setLoadingDiscover(false);
    }
  };

  const handleDiscover = async () => {
    if (!isLoopbackOrPrivateUrl(agentUrl)) {
      setPendingAction("discover");
      return;
    }
    await runDiscover();
  };

  const runSendTask = async () => {
    if (!taskText.trim()) return;
    setLoadingTask(true);
    try {
      const req = {
        id: crypto.randomUUID(),
        sessionId: null,
        message: {
          role: "user",
          parts: [{ type: "text", text: taskText }],
          metadata: taskSkill ? { skill: taskSkill } : undefined,
        },
        acceptedOutputModes: ["text"],
        pushNotification: null,
        historyLength: null,
        metadata: taskSkill ? { skill: taskSkill } : null,
      };
      const resp = await a2aSendTask(agentUrl, JSON.stringify(req));
      setTaskResult(resp);
    } catch (e) {
      setTaskResult("错误: " + String(e));
    } finally {
      setLoadingTask(false);
    }
  };

  const handleSendTask = async () => {
    if (!taskText.trim()) return;
    if (!isLoopbackOrPrivateUrl(agentUrl)) {
      setPendingAction("send");
      return;
    }
    await runSendTask();
  };

  const handleLocalService = async (skill: string, text: string) => {
    setLoadingLocal(true);
    setActiveLocalSkill(skill);
    try {
      const req = {
        id: crypto.randomUUID(),
        sessionId: null,
        message: {
          role: "user",
          parts: [{ type: "text", text }],
          metadata: { skill },
        },
        acceptedOutputModes: ["text"],
        pushNotification: null,
        historyLength: null,
        metadata: { skill },
      };
      const resp = await a2aHandleTask(JSON.stringify(req));
      setLocalResult(resp);
    } catch (e) {
      setLocalResult("错误: " + String(e));
    } finally {
      setLoadingLocal(false);
      setActiveLocalSkill(null);
    }
  };

  const handleBridge = async () => {
    if (!bridgeInput.trim()) return;
    setBridgeLoading(true);
    try {
      const result = await a2aBridgeLocal(
        bridgeMethod,
        bridgeInput,
        "local-bridge-demo",
        "openlife.reasoning_bridge"
      );
      setBridgeResult(JSON.stringify(result, null, 2));
    } catch (e) {
      setBridgeResult("错误: " + String(e));
    } finally {
      setBridgeLoading(false);
    }
  };

  const runRestartSidecar = async () => {
    try {
      await a2aRestartSidecar();
      setSidecarMsg("A2A sidecar 已重启");
      await refreshLocalCard();
    } catch (e) {
      setSidecarMsg("重启失败: " + String(e));
    }
  };

  const runStopSidecar = async () => {
    try {
      await a2aStopSidecar();
      setSidecarMsg("A2A sidecar 已停止");
    } catch (e) {
      setSidecarMsg("停止失败: " + String(e));
    }
  };

  const confirmPendingAction = async () => {
    const action = pendingAction;
    setPendingAction(null);
    if (action === "discover") await runDiscover();
    if (action === "send") await runSendTask();
    if (action === "restart") await runRestartSidecar();
    if (action === "stop") await runStopSidecar();
  };

  const pendingCopy: Record<PendingA2AAction, { title: string; description: string; label: string }> = {
    discover: {
      title: "确认发现外部 A2A Agent",
      description: `将向 ${agentUrl} 发起网络请求读取 Agent Card。`,
      label: "发现外部 Agent",
    },
    send: {
      title: "确认发送 A2A Task",
      description: `将向 ${agentUrl} 发送当前任务内容。请确认目标 Agent 可信。`,
      label: "发送 Task",
    },
    restart: {
      title: "确认重启 A2A Sidecar",
      description: "这会重启本地 A2A sidecar，当前连接可能短暂中断。",
      label: "重启 Sidecar",
    },
    stop: {
      title: "确认停止 A2A Sidecar",
      description: "这会停止本地 A2A sidecar，本地 A2A 服务将不可用，直到再次启动。",
      label: "停止 Sidecar",
    },
  };
  const pending = pendingAction ? pendingCopy[pendingAction] : null;

  return (
    <div className="h-full overflow-auto bg-white p-6">
      <ConfirmDangerDialog
        open={Boolean(pending)}
        title={pending?.title ?? ""}
        description={pending?.description ?? ""}
        confirmLabel={pending?.label ?? "确认"}
        severity={pendingAction === "stop" ? "danger" : "warning"}
        busy={loadingDiscover || loadingTask || bridgeLoading || loadingLocal}
        onConfirm={() => void confirmPendingAction()}
        onCancel={() => setPendingAction(null)}
      />
      <div className="max-w-4xl mx-auto space-y-8">
        <ErrorBanner message={pageError} onClose={() => setPageError("")} />
        <h2 className="text-xl font-bold text-gray-900 flex items-center gap-2">
          <Network className="text-indigo-600" size={22} />
          A2A 协议适配
        </h2>

        <section className="space-y-3">
          <h3 className="text-sm font-semibold text-gray-700">A2A Client - 发现外部 Agent</h3>
          <div className="flex gap-2">
            <input
              value={agentUrl}
              onChange={e => setAgentUrl(e.target.value)}
              placeholder="Agent Base URL"
              className="flex-1 border rounded-lg px-3 py-2 text-sm"
            />
            <button
              onClick={handleDiscover}
              disabled={loadingDiscover}
              className="bg-indigo-600 text-white px-4 py-2 rounded-lg text-sm hover:bg-indigo-700 disabled:opacity-50 flex items-center gap-2"
            >
              {loadingDiscover ? (
                <RefreshCw size={16} className="animate-spin" />
              ) : (
                <Network size={16} />
              )}
              发现
            </button>
          </div>
          {agentCard && (
            <div className="bg-gray-50 border rounded-lg p-4 text-sm space-y-1">
              <div>
                <span className="font-medium">Name:</span> {agentCard.name}
              </div>
              <div>
                <span className="font-medium">Description:</span> {agentCard.description}
              </div>
              <div>
                <span className="font-medium">Version:</span> {agentCard.version}
              </div>
              <div>
                <span className="font-medium">URL:</span> {agentCard.url}
              </div>
              <div>
                <span className="font-medium">Capabilities:</span>{" "}
                {JSON.stringify(agentCard.capabilities)}
              </div>
              <div>
                <span className="font-medium">Skills:</span>
              </div>
              <ul className="list-disc list-inside text-gray-700">
                {agentCard.skills?.map((s: any) => (
                  <li key={s.id}>
                    {s.name} — {s.description}
                  </li>
                ))}
              </ul>
            </div>
          )}
        </section>

        <section className="space-y-3">
          <h3 className="text-sm font-semibold text-gray-700">A2A Client - 发送 Task</h3>
          <div className="flex gap-2">
            <input
              value={taskSkill}
              onChange={e => setTaskSkill(e.target.value)}
              placeholder="Skill metadata (可选)"
              className="w-40 border rounded-lg px-3 py-2 text-sm"
            />
            <input
              value={taskText}
              onChange={e => setTaskText(e.target.value)}
              placeholder="输入要发送给 Agent 的内容"
              className="flex-1 border rounded-lg px-3 py-2 text-sm"
            />
            <button
              onClick={handleSendTask}
              disabled={loadingTask || !taskText.trim()}
              className="bg-indigo-600 text-white px-4 py-2 rounded-lg text-sm hover:bg-indigo-700 disabled:opacity-50 flex items-center gap-2"
            >
              {loadingTask ? <RefreshCw size={16} className="animate-spin" /> : <Send size={16} />}
              发送
            </button>
          </div>
          {taskResult && (
            <StructuredJsonResult value={taskResult} />
          )}
        </section>

        <section className="space-y-3">
          <h3 className="text-sm font-semibold text-gray-700">A2A Server - OpenLife 本地服务</h3>
          <div className="flex items-center gap-2">
            <button
              onClick={() => setPendingAction("restart")}
              className="bg-white border px-3 py-2 rounded-lg text-sm hover:bg-gray-50 flex items-center gap-2"
            >
              <Play size={16} /> 重启 Sidecar
            </button>
            <button
              onClick={() => setPendingAction("stop")}
              className="bg-white border px-3 py-2 rounded-lg text-sm hover:bg-gray-50 flex items-center gap-2"
            >
              <Square size={16} /> 停止 Sidecar
            </button>
            {sidecarMsg && <div className="text-xs text-gray-500">{sidecarMsg}</div>}
          </div>
          {localCard && (
            <div className="bg-indigo-50 border border-indigo-100 rounded-lg p-4 text-sm space-y-1">
              <div>
                <span className="font-medium">Name:</span> {localCard.name}
              </div>
              <div>
                <span className="font-medium">Description:</span> {localCard.description}
              </div>
              <div>
                <span className="font-medium">Skills:</span>
              </div>
              <ul className="list-disc list-inside text-gray-700">
                {localCard.skills?.map((s: any) => (
                  <li key={s.id}>
                    {s.name} — {s.description}
                  </li>
                ))}
              </ul>
            </div>
          )}
          <div className="flex flex-col gap-3">
            <div className="flex gap-2">
	              <input
	                value={localServiceInput}
	                onChange={e => setLocalServiceInput(e.target.value)}
	                placeholder="输入本地固定技能的查询内容"
	                className="flex-1 border rounded-lg px-3 py-2 text-sm"
	              />
	              <button
	                onClick={() =>
	                  handleLocalService(
	                    "openlife.query_life_model",
	                    localServiceInput || "查询人生模型"
	                  )
	                }
                disabled={loadingLocal}
                className="bg-white border px-3 py-2 rounded-lg text-sm hover:bg-gray-50 flex items-center gap-2"
              >
                <Sparkles size={16} /> 查询
              </button>
            </div>
            <div className="flex gap-2">
              <button
	                onClick={() =>
	                  handleLocalService("openlife.assess_values", localServiceInput || "评估这段话")
	                }
                disabled={loadingLocal}
                className="bg-white border px-3 py-2 rounded-lg text-sm hover:bg-gray-50 flex items-center gap-2"
              >
                <Shield size={16} /> 价值观评估
              </button>
              <button
	                onClick={() =>
	                  handleLocalService(
	                    "openlife.reasoning_bridge",
	                    localServiceInput || "帮我做决策"
	                  )
	                }
                disabled={loadingLocal}
                className="bg-white border px-3 py-2 rounded-lg text-sm hover:bg-gray-50 flex items-center gap-2"
              >
                <Network size={16} /> 推理桥接
              </button>
            </div>
            {loadingLocal && activeLocalSkill && (
              <div className="text-xs text-gray-500 flex items-center gap-2">
                <RefreshCw size={14} className="animate-spin" />
                正在调用 {activeLocalSkill}...
              </div>
            )}
          </div>
          {localResult && (
            <StructuredJsonResult value={localResult} />
          )}
        </section>

        <section className="space-y-3">
          <h3 className="text-sm font-semibold text-gray-700">OpenLife ↔ A2A 桥接调试</h3>
          <div className="flex gap-2">
            <input
              value={bridgeMethod}
              onChange={e => setBridgeMethod(e.target.value)}
              placeholder="推理方法"
              className="w-40 border rounded-lg px-3 py-2 text-sm"
            />
	            <input
	              value={bridgeInput}
	              onChange={e => setBridgeInput(e.target.value)}
	              placeholder="输入要送入 OpenLife/A2A 桥接的文本"
              className="flex-1 border rounded-lg px-3 py-2 text-sm"
            />
	            <button
	              onClick={handleBridge}
	              disabled={bridgeLoading || !bridgeInput.trim()}
              className="bg-indigo-600 text-white px-4 py-2 rounded-lg text-sm hover:bg-indigo-700 disabled:opacity-50 flex items-center gap-2"
            >
              {bridgeLoading ? (
                <RefreshCw size={16} className="animate-spin" />
              ) : (
                <Network size={16} />
              )}
              桥接运行
            </button>
          </div>
          <div className="text-xs text-gray-500">
            这个区域会展示 OpenLife 请求如何映射成 A2A Task，以及 A2A 响应如何重新转换回 OpenLife
            结果。
          </div>
          {bridgeResult && (
            <StructuredJsonResult value={bridgeResult} />
          )}
        </section>
      </div>
    </div>
  );
}
