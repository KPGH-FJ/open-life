import type { ReactNode } from "react";
import { useEffect, useId, useState } from "react";
import { AlertTriangle, X } from "lucide-react";

type ConfirmDangerDialogProps = {
  open: boolean;
  title: string;
  description: ReactNode;
  confirmLabel?: string;
  cancelLabel?: string;
  severity?: "warning" | "danger";
  confirmationText?: string;
  busy?: boolean;
  onConfirm: () => void | Promise<void>;
  onCancel: () => void;
};

export default function ConfirmDangerDialog({
  open,
  title,
  description,
  confirmLabel = "确认",
  cancelLabel = "取消",
  severity = "danger",
  confirmationText,
  busy = false,
  onConfirm,
  onCancel,
}: ConfirmDangerDialogProps) {
  const [typedText, setTypedText] = useState("");
  const requiresTypedConfirmation = Boolean(confirmationText);
  const canConfirm = !busy && (!requiresTypedConfirmation || typedText === confirmationText);
  const titleId = useId();

  useEffect(() => {
    if (open) {
      setTypedText("");
    }
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) {
        onCancel();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [busy, onCancel, open]);

  if (!open) return null;

  const tone =
    severity === "danger"
      ? "border-rose-200 bg-rose-50 text-rose-800"
      : "border-amber-200 bg-amber-50 text-amber-800";
  const buttonTone =
    severity === "danger"
      ? "bg-rose-700 text-white hover:bg-rose-800"
      : "bg-amber-700 text-white hover:bg-amber-800";

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-4"
      role="presentation"
    >
      <section
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className="w-full max-w-md rounded-lg border border-stone-200 bg-white shadow-xl"
      >
        <div className="flex items-start gap-3 border-b border-stone-100 px-4 py-4">
          <div className={`mt-0.5 rounded-md border p-2 ${tone}`}>
            <AlertTriangle size={18} aria-hidden="true" />
          </div>
          <div className="min-w-0 flex-1">
            <h2 id={titleId} className="text-base font-semibold text-stone-950">
              {title}
            </h2>
            <div className="mt-1 text-sm leading-6 text-stone-600">{description}</div>
          </div>
          <button
            type="button"
            aria-label="关闭确认对话框"
            onClick={onCancel}
            disabled={busy}
            className="rounded-md p-1 text-stone-400 hover:bg-stone-100 hover:text-stone-700 disabled:opacity-50"
          >
            <X size={16} aria-hidden="true" />
          </button>
        </div>

        {confirmationText && (
          <div className="border-b border-stone-100 px-4 py-3">
            <label className="block text-xs font-medium text-stone-600">
              输入 {confirmationText} 以继续
              <input
                value={typedText}
                onChange={event => setTypedText(event.target.value)}
                className="mt-2 w-full rounded-md border border-stone-300 px-3 py-2 text-sm text-stone-900 focus:outline-none focus:ring-2 focus:ring-stone-900/20"
                autoFocus
              />
            </label>
          </div>
        )}

        <div className="flex justify-end gap-2 px-4 py-3">
          <button
            type="button"
            onClick={onCancel}
            disabled={busy}
            className="rounded-md border border-stone-200 bg-white px-3 py-2 text-sm font-medium text-stone-700 hover:bg-stone-50 disabled:opacity-50"
          >
            {cancelLabel}
          </button>
          <button
            type="button"
            onClick={onConfirm}
            disabled={!canConfirm}
            className={`rounded-md px-3 py-2 text-sm font-semibold disabled:cursor-not-allowed disabled:opacity-50 ${buttonTone}`}
          >
            {busy ? "处理中..." : confirmLabel}
          </button>
        </div>
      </section>
    </div>
  );
}
