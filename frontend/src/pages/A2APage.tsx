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
  const [localInput, setLocalInput] = useState("");
  const [activeLocalSkill, setActiveLocalSkill] = useState<string | null>(null);
  const [bridgeMethod, setBridgeMethod] = useState("a2a.send");
  const [bridgeResult, setBridgeResult] = useState<string | null>(null);
  const [bridgeLoading, setBridgeLoading] = useState(false);
  const [sidecarMsg, setSidecarMsg] = useState<string | null>(null);
  const [pageError, setPageError] = useState<string>("");

  useEffect(() => {
    a2aLocalAgentCard()
      .then(setLocalCard)
      .catch(() => {});
  }, []);

  const refreshLocalCard = async () => {
    const card = await a2aLocalAgentCard();
    setLocalCard(card);
  };

  const handleDiscover = async () => {
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

  const handleSendTask = async () => {
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
    if (!localInput.trim()) return;
    setBridgeLoading(true);
    try {
      const result = await a2aBridgeLocal(
        bridgeMethod,
        localInput,
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

  const handleRestartSidecar = async () => {
    try {
      await a2aRestartSidecar();
      setSidecarMsg("A2A sidecar 已重启");
      await refreshLocalCard();
    } catch (e) {
      setSidecarMsg("重启失败: " + String(e));
    }
  };

  const handleStopSidecar = async () => {
    try {
      await a2aStopSidecar();
      setSidecarMsg("A2A sidecar 已停止");
    } catch (e) {
      setSidecarMsg("停止失败: " + String(e));
    }
  };

  return (
    <div className="h-full overflow-auto bg-white p-6">
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
            <pre className="bg-gray-50 border rounded-lg p-4 text-xs overflow-auto max-h-64">
              {taskResult}
            </pre>
          )}
        </section>

        <section className="space-y-3">
          <h3 className="text-sm font-semibold text-gray-700">A2A Server - OpenLife 本地服务</h3>
          <div className="flex items-center gap-2">
            <button
              onClick={handleRestartSidecar}
              className="bg-white border px-3 py-2 rounded-lg text-sm hover:bg-gray-50 flex items-center gap-2"
            >
              <Play size={16} /> 重启 Sidecar
            </button>
            <button
              onClick={handleStopSidecar}
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
                value={localInput}
                onChange={e => setLocalInput(e.target.value)}
                placeholder="输入要发给本地 A2A Agent 的内容"
                className="flex-1 border rounded-lg px-3 py-2 text-sm"
              />
              <button
                onClick={() =>
                  handleLocalService("openlife.query_life_model", localInput || "查询人生模型")
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
                  handleLocalService("openlife.assess_values", localInput || "评估这段话")
                }
                disabled={loadingLocal}
                className="bg-white border px-3 py-2 rounded-lg text-sm hover:bg-gray-50 flex items-center gap-2"
              >
                <Shield size={16} /> 价值观评估
              </button>
              <button
                onClick={() =>
                  handleLocalService("openlife.reasoning_bridge", localInput || "帮我做决策")
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
            <pre className="bg-gray-50 border rounded-lg p-4 text-xs overflow-auto max-h-64">
              {localResult}
            </pre>
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
              value={localInput}
              onChange={e => setLocalInput(e.target.value)}
              placeholder="输入要送入 OpenLife/A2A 桥接的文本"
              className="flex-1 border rounded-lg px-3 py-2 text-sm"
            />
            <button
              onClick={handleBridge}
              disabled={bridgeLoading || !localInput.trim()}
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
            <pre className="bg-gray-50 border rounded-lg p-4 text-xs overflow-auto max-h-80">
              {bridgeResult}
            </pre>
          )}
        </section>
      </div>
    </div>
  );
}
