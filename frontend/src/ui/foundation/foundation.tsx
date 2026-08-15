import {
  forwardRef,
  useEffect,
  useId,
  useRef,
  type ButtonHTMLAttributes,
  type InputHTMLAttributes,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import { LoaderCircle, X } from "lucide-react";

export type FoundationActionVariant = "primary" | "secondary" | "quiet" | "danger";
export type FoundationStatus =
  | "neutral"
  | "waiting"
  | "stale"
  | "unknown"
  | "blocked"
  | "error"
  | "success";

function cx(...classes: Array<string | false | null | undefined>): string {
  return classes.filter(Boolean).join(" ");
}

function requireDisabledReason(disabled: boolean, disabledReason?: string): string | undefined {
  if (!disabled) return undefined;
  if (!disabledReason?.trim()) {
    throw new Error("Disabled OpenLife controls require a visible disabledReason.");
  }
  return disabledReason.trim();
}

export type FoundationActionButtonProps = Omit<
  ButtonHTMLAttributes<HTMLButtonElement>,
  "children"
> & {
  label: string;
  icon?: ReactNode;
  variant?: FoundationActionVariant;
  loading?: boolean;
  loadingLabel?: string;
  disabledReason?: string;
};

export const FoundationActionButton = forwardRef<HTMLButtonElement, FoundationActionButtonProps>(
  function FoundationActionButton(
    {
      label,
      icon,
      variant = "secondary",
      loading = false,
      loadingLabel = "处理中",
      disabled = false,
      disabledReason,
      className,
      type = "button",
      ...buttonProps
    },
    ref
  ) {
    const reasonId = useId();
    const reason = requireDisabledReason(disabled, disabledReason);
    const unavailable = disabled || loading;

    return (
      <span className="ol-control-stack">
        <button
          {...buttonProps}
          ref={ref}
          type={type}
          disabled={unavailable}
          aria-busy={loading || undefined}
          aria-describedby={reason ? reasonId : buttonProps["aria-describedby"]}
          className={cx("ol-action-button", `ol-action-button--${variant}`, className)}
        >
          <span className="ol-action-button__content">
            {loading ? <LoaderCircle className="ol-spinner" size={18} aria-hidden="true" /> : icon}
            <span>{loading ? loadingLabel : label}</span>
          </span>
        </button>
        {reason && (
          <span id={reasonId} className="ol-disabled-reason">
            {reason}
          </span>
        )}
      </span>
    );
  }
);

export type FoundationIconButtonProps = Omit<
  ButtonHTMLAttributes<HTMLButtonElement>,
  "children" | "aria-label" | "title"
> & {
  label: string;
  icon: ReactNode;
  disabledReason?: string;
};

export const FoundationIconButton = forwardRef<HTMLButtonElement, FoundationIconButtonProps>(
  function FoundationIconButton(
    { label, icon, disabled = false, disabledReason, className, type = "button", ...buttonProps },
    ref
  ) {
    const reasonId = useId();
    const reason = requireDisabledReason(disabled, disabledReason);
    return (
      <span className="ol-icon-control">
        <button
          {...buttonProps}
          ref={ref}
          type={type}
          disabled={disabled}
          aria-label={label}
          aria-describedby={reason ? reasonId : undefined}
          title={label}
          className={cx("ol-icon-button", className)}
        >
          {icon}
        </button>
        {reason && (
          <span id={reasonId} className="ol-disabled-reason">
            {reason}
          </span>
        )}
      </span>
    );
  }
);

export function FoundationStatusLabel({
  label,
  status = "neutral",
  verified = false,
  live = false,
}: {
  label: string;
  status?: FoundationStatus;
  verified?: boolean;
  live?: boolean;
}) {
  if (status === "success" && !verified) {
    throw new Error("Success status requires verified=true.");
  }
  return (
    <span
      className={cx("ol-status-label", `ol-status-label--${status}`)}
      role={live ? "status" : undefined}
    >
      <span className="ol-status-label__dot" aria-hidden="true" />
      {label}
    </span>
  );
}

export function FoundationNotice({
  title,
  children,
  tone = "protection",
  live = false,
}: {
  title: string;
  children: ReactNode;
  tone?: "protection" | "error" | "neutral";
  live?: boolean;
}) {
  return (
    <section
      className={cx("ol-notice", `ol-notice--${tone}`)}
      role={tone === "error" ? "alert" : live ? "status" : undefined}
    >
      <div className="ol-notice__title">{title}</div>
      <div className="ol-notice__body">{children}</div>
    </section>
  );
}

export type FoundationTextFieldProps = Omit<
  InputHTMLAttributes<HTMLInputElement>,
  "id" | "aria-describedby"
> & {
  id: string;
  label: string;
  description?: string;
  error?: string;
  stateMessage?: string;
  disabledReason?: string;
};

export function FoundationTextField({
  id,
  label,
  description,
  error,
  stateMessage,
  disabled = false,
  disabledReason,
  className,
  ...inputProps
}: FoundationTextFieldProps) {
  const descriptionId = `${id}-description`;
  const errorId = `${id}-error`;
  const stateId = `${id}-state`;
  const reasonId = `${id}-disabled-reason`;
  const reason = requireDisabledReason(disabled, disabledReason);
  const describedBy = [
    description && descriptionId,
    stateMessage && stateId,
    error && errorId,
    reason && reasonId,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className="ol-field">
      <label className="ol-field__label" htmlFor={id}>
        {label}
      </label>
      {description && (
        <span id={descriptionId} className="ol-field__description">
          {description}
        </span>
      )}
      <input
        {...inputProps}
        id={id}
        disabled={disabled}
        aria-invalid={Boolean(error) || undefined}
        aria-describedby={describedBy || undefined}
        className={cx("ol-text-field", error && "ol-text-field--error", className)}
      />
      {stateMessage && (
        <span id={stateId} className="ol-field__state">
          {stateMessage}
        </span>
      )}
      {error && (
        <span id={errorId} className="ol-field__error">
          {error}
        </span>
      )}
      {reason && (
        <span id={reasonId} className="ol-disabled-reason">
          {reason}
        </span>
      )}
    </div>
  );
}

export function FoundationToggle({
  label,
  description,
  state,
  onChange,
  disabled = false,
  disabledReason,
}: {
  label: string;
  description?: string;
  state: "on" | "off" | "unknown";
  onChange?: (next: "on" | "off") => void;
  disabled?: boolean;
  disabledReason?: string;
}) {
  const reasonId = useId();
  const reason = requireDisabledReason(disabled, disabledReason);

  if (state === "unknown") {
    return (
      <div className="ol-toggle-row ol-toggle-row--unknown">
        <span className="ol-toggle-copy">
          <span className="ol-toggle-copy__label">{label}</span>
          {description && <span className="ol-toggle-copy__description">{description}</span>}
        </span>
        <span className="ol-toggle-unknown">状态未知</span>
      </div>
    );
  }

  return (
    <div className="ol-toggle-row">
      <span className="ol-toggle-copy">
        <span className="ol-toggle-copy__label">{label}</span>
        {description && <span className="ol-toggle-copy__description">{description}</span>}
        {reason && (
          <span id={reasonId} className="ol-disabled-reason">
            {reason}
          </span>
        )}
      </span>
      <button
        type="button"
        role="switch"
        aria-checked={state === "on"}
        aria-label={label}
        aria-describedby={reason ? reasonId : undefined}
        disabled={disabled}
        className="ol-toggle"
        data-state={state}
        onClick={() => onChange?.(state === "on" ? "off" : "on")}
      >
        <span className="ol-toggle__track" aria-hidden="true">
          <span className="ol-toggle__thumb" />
        </span>
      </button>
    </div>
  );
}

export function FoundationNavRow({
  label,
  meta,
  icon,
  current = false,
  badge,
  onClick,
}: {
  label: string;
  meta?: string;
  icon: ReactNode;
  current?: boolean;
  badge?: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className={cx("ol-nav-row", current && "ol-nav-row--current")}
      aria-current={current ? "page" : undefined}
      aria-label={meta ? `${label} ${meta}` : label}
      onClick={onClick}
    >
      <span className="ol-nav-row__icon" aria-hidden="true">
        {icon}
      </span>
      <span className="ol-nav-row__copy">
        <span className="ol-nav-row__label">{label}</span>
        {meta && <span className="ol-nav-row__meta">{meta}</span>}
      </span>
      {badge && <span className="ol-nav-row__badge">{badge}</span>}
    </button>
  );
}

export function FoundationEvidenceRow({
  id,
  label,
  source,
  sensitivity,
  onOpen,
}: {
  id: string;
  label: string;
  source: string;
  sensitivity: string;
  onOpen: () => void;
}) {
  return (
    <button type="button" className="ol-evidence-row" data-evidence-id={id} onClick={onOpen}>
      <span className="ol-evidence-row__label">{label}</span>
      <span className="ol-evidence-row__meta">
        {source} · {sensitivity}
      </span>
    </button>
  );
}

function focusableElements(root: HTMLElement): HTMLElement[] {
  return Array.from(
    root.querySelectorAll<HTMLElement>(
      'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [href], [tabindex]:not([tabindex="-1"])'
    )
  ).filter(element => !element.hasAttribute("hidden"));
}

export function FoundationDialog({
  open,
  title,
  description,
  children,
  footer,
  busy = false,
  onClose,
}: {
  open: boolean;
  title: string;
  description?: string;
  children?: ReactNode;
  footer: ReactNode;
  busy?: boolean;
  onClose: () => void;
}) {
  const titleId = useId();
  const descriptionId = useId();
  const dialogRef = useRef<HTMLDivElement>(null);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const closeStateRef = useRef({ busy, onClose });
  closeStateRef.current = { busy, onClose };

  useEffect(() => {
    if (!open) return;
    const previouslyFocused = document.activeElement as HTMLElement | null;
    const dialog = dialogRef.current;
    const appRoot = document.getElementById("root");
    const previousAriaHidden = appRoot?.getAttribute("aria-hidden");
    const previousInert = appRoot?.inert ?? false;
    if (appRoot) {
      appRoot.inert = true;
      appRoot.setAttribute("aria-hidden", "true");
    }
    headingRef.current?.focus();

    function onKeyDown(event: KeyboardEvent) {
      if (!dialog) return;
      if (event.key === "Escape") {
        if (!closeStateRef.current.busy) {
          event.preventDefault();
          closeStateRef.current.onClose();
        }
        return;
      }
      if (event.key !== "Tab") return;
      const focusable = focusableElements(dialog);
      if (focusable.length === 0) {
        event.preventDefault();
        headingRef.current?.focus();
        return;
      }
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (document.activeElement === headingRef.current) {
        event.preventDefault();
        (event.shiftKey ? last : first).focus();
      } else if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    }

    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      if (appRoot) {
        appRoot.inert = previousInert;
        if (previousAriaHidden == null) appRoot.removeAttribute("aria-hidden");
        else appRoot.setAttribute("aria-hidden", previousAriaHidden);
      }
      previouslyFocused?.focus();
    };
  }, [open]);

  if (!open) return null;

  return createPortal(
    <div className="ol-foundation ol-dialog-backdrop">
      <div
        ref={dialogRef}
        className="ol-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={description ? descriptionId : undefined}
      >
        <header className="ol-dialog__header">
          <div>
            <h2 ref={headingRef} id={titleId} className="ol-dialog__title" tabIndex={-1}>
              {title}
            </h2>
            {description && (
              <p id={descriptionId} className="ol-dialog__description">
                {description}
              </p>
            )}
          </div>
          <FoundationIconButton
            label="关闭对话框"
            icon={<X size={18} aria-hidden="true" />}
            disabled={busy}
            disabledReason={busy ? "正在提交，暂时不能关闭。" : undefined}
            onClick={onClose}
          />
        </header>
        {children && <div className="ol-dialog__body">{children}</div>}
        <footer className="ol-dialog__footer">{footer}</footer>
      </div>
    </div>,
    document.body
  );
}

export function FoundationLiveRegion({ message }: { message: string }) {
  return (
    <div className="ol-sr-only" aria-live="polite" aria-atomic="true">
      {message}
    </div>
  );
}
