import { useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  Sparkles,
  KeyRound,
  BrainCircuit,
  ShieldCheck,
  ChevronRight,
  ChevronLeft,
  Rocket,
  Settings,
  Hammer,
} from "lucide-react";
import { markOnboardingCompleted } from "../tauri";

interface Props {
  onComplete: () => void;
}

export default function OnboardingWizard({ onComplete }: Props) {
  const [step, setStep] = useState(0);
  const navigate = useNavigate();
  const totalSteps = 4;

  const handleFinish = async () => {
    try {
      await markOnboardingCompleted();
    } catch {
      // ignore
    }
    onComplete();
  };

  const goSettings = () => {
    handleFinish();
    navigate("/settings");
  };

  const goBuilder = () => {
    handleFinish();
    navigate("/builder");
  };

  const steps = [
    {
      title: "欢迎使用 OpenLife",
      icon: <Sparkles size={32} className="text-indigo-600" />,
      content: (
        <div className="space-y-4">
          <p className="text-gray-700 leading-relaxed">
            OpenLife 是你的终身成长合伙人。通过四维人生模型（身份、目标、能力、状态）与 AI
            持续对话，你会获得越来越贴合个人语境的建议。
          </p>
          <div className="rounded-lg bg-indigo-50 border border-indigo-100 p-4 text-sm text-indigo-800">
            <p className="font-medium mb-1">Beta 试用提示</p>
            <p>
              当前为 Beta 版本，建议先完成基础配置和人生模型初始化，以获得最佳体验。
            </p>
          </div>
        </div>
      ),
    },
    {
      title: "配置对话后端",
      icon: <KeyRound size={32} className="text-indigo-600" />,
      content: (
        <div className="space-y-4">
          <p className="text-gray-700 leading-relaxed">
            OpenLife 支持两种对话后端，至少需要配置一种才能使用聊天功能：
          </p>
          <ul className="list-disc list-inside text-sm text-gray-700 space-y-2">
            <li>
              <strong>云端模型</strong>（推荐）：在设置页填写 OpenRouter 或 OpenAI 的 API Key，响应质量高且支持工具调用。
            </li>
            <li>
              <strong>本地模型</strong>：安装 Ollama 并在本地运行模型，数据完全本地保留，但工具调用能力有限。
            </li>
          </ul>
          <button
            onClick={goSettings}
            className="inline-flex items-center gap-2 px-4 py-2 rounded-md bg-indigo-600 text-white text-sm font-medium hover:bg-indigo-700 transition"
          >
            <Settings size={16} />
            前往设置页配置
          </button>
        </div>
      ),
    },
    {
      title: "构建你的人生模型",
      icon: <BrainCircuit size={32} className="text-indigo-600" />,
      content: (
        <div className="space-y-4">
          <p className="text-gray-700 leading-relaxed">
            人生模型是 OpenLife 理解你的核心。它包括你的价值观、目标、技能与当前状态。
            你可以通过「构建」向导快速生成初始模型，也可以稍后手动编辑。
          </p>
          <div className="rounded-lg bg-amber-50 border border-amber-100 p-4 text-sm text-amber-800">
            <p>
              建议至少完成一次快速构建（约 3-5 分钟），这样 AI 才能基于你的真实背景给出建议。
            </p>
          </div>
          <button
            onClick={goBuilder}
            className="inline-flex items-center gap-2 px-4 py-2 rounded-md bg-indigo-600 text-white text-sm font-medium hover:bg-indigo-700 transition"
          >
            <Hammer size={16} />
            前往构建向导
          </button>
        </div>
      ),
    },
    {
      title: "隐私与数据安全",
      icon: <ShieldCheck size={32} className="text-indigo-600" />,
      content: (
        <div className="space-y-4">
          <p className="text-gray-700 leading-relaxed">
            你的隐私对我们至关重要：
          </p>
          <ul className="list-disc list-inside text-sm text-gray-700 space-y-2">
            <li>
              所有对话记录、人生模型快照和记忆向量均保存在本地 SQLite 数据库中。
            </li>
            <li>
              如果选择云端模型，仅对话内容会发送到对应的 API 服务商（OpenRouter / OpenAI），不会经过第三方中转。
            </li>
            <li>
              内置 PII 检测引擎会自动识别身份证号、银行卡号等敏感信息，在发送前脱敏或拦截。
            </li>
            <li>
              你可以在「设置 → 隐私策略」中自定义检测规则。
            </li>
          </ul>
          <button
            onClick={handleFinish}
            className="inline-flex items-center gap-2 px-4 py-2 rounded-md bg-indigo-600 text-white text-sm font-medium hover:bg-indigo-700 transition"
          >
            <Rocket size={16} />
            开始使用 OpenLife
          </button>
        </div>
      ),
    },
  ];

  const current = steps[step];

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm p-4">
      <div className="w-full max-w-lg bg-white rounded-2xl shadow-xl overflow-hidden">
        <div className="px-6 pt-6 pb-4">
          <div className="flex items-center gap-3 mb-2">
            <div className="p-2 rounded-lg bg-indigo-50">{current.icon}</div>
            <h2 className="text-xl font-bold text-gray-900">{current.title}</h2>
          </div>
          <div className="mt-4">{current.content}</div>
        </div>

        <div className="px-6 py-4 bg-gray-50 border-t flex items-center justify-between">
          <div className="flex gap-1">
            {Array.from({ length: totalSteps }).map((_, i) => (
              <div
                key={i}
                className={`h-2 w-8 rounded-full transition ${
                  i === step ? "bg-indigo-600" : "bg-gray-300"
                }`}
              />
            ))}
          </div>
          <div className="flex gap-2">
            {step > 0 && (
              <button
                onClick={() => setStep((s) => s - 1)}
                className="inline-flex items-center gap-1 px-3 py-2 rounded-md text-sm font-medium text-gray-700 hover:bg-gray-200 transition"
              >
                <ChevronLeft size={16} />
                上一步
              </button>
            )}
            {step < totalSteps - 1 ? (
              <button
                onClick={() => setStep((s) => s + 1)}
                className="inline-flex items-center gap-1 px-4 py-2 rounded-md bg-indigo-600 text-white text-sm font-medium hover:bg-indigo-700 transition"
              >
                下一步
                <ChevronRight size={16} />
              </button>
            ) : null}
          </div>
        </div>
      </div>
    </div>
  );
}
