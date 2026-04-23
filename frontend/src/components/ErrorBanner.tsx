import { useEffect } from "react";
import { AlertCircle, AlertTriangle, Info, X } from "lucide-react";

interface Props {
  message: string;
  severity?: "error" | "warning" | "info";
  onClose?: () => void;
  autoHide?: boolean;
  autoHideMs?: number;
  className?: string;
}

const styles = {
  error: "bg-rose-50 border-rose-200 text-rose-800",
  warning: "bg-amber-50 border-amber-200 text-amber-800",
  info: "bg-blue-50 border-blue-200 text-blue-800",
};

const iconMap = {
  error: AlertCircle,
  warning: AlertTriangle,
  info: Info,
};

export default function ErrorBanner({
  message,
  severity = "error",
  onClose,
  autoHide = false,
  autoHideMs = 4000,
  className = "",
}: Props) {
  useEffect(() => {
    if (!autoHide || !onClose) return;
    const timer = setTimeout(onClose, autoHideMs);
    return () => clearTimeout(timer);
  }, [autoHide, autoHideMs, onClose, message]);

  const Icon = iconMap[severity];

  if (!message) return null;

  return (
    <div
      className={`flex items-start gap-2 rounded-lg border px-3 py-2 text-sm ${styles[severity]} ${className}`}
      role="alert"
      data-testid="error-banner"
    >
      <Icon size={16} className="mt-0.5 shrink-0 opacity-80" />
      <span className="flex-1">{message}</span>
      {onClose && (
        <button
          onClick={onClose}
          className="shrink-0 rounded p-0.5 hover:bg-black/5 transition-colors"
          aria-label="关闭"
        >
          <X size={14} />
        </button>
      )}
    </div>
  );
}
