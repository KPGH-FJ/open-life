import type { ReactNode } from "react";

export type ProductTone = "neutral" | "ready" | "warning" | "danger" | "info";

const toneClasses: Record<ProductTone, string> = {
  neutral: "border-stone-200 bg-white text-stone-700",
  ready: "border-emerald-200 bg-emerald-50 text-emerald-800",
  warning: "border-amber-200 bg-amber-50 text-amber-900",
  danger: "border-rose-200 bg-rose-50 text-rose-800",
  info: "border-sky-200 bg-sky-50 text-sky-800",
};

function cx(...classes: Array<string | false | null | undefined>): string {
  return classes.filter(Boolean).join(" ");
}

export function StatusChip({
  label,
  tone = "neutral",
  title,
}: {
  label: string;
  tone?: ProductTone;
  title?: string;
}) {
  return (
    <span
      title={title}
      className={cx(
        "inline-flex min-h-6 items-center rounded-md border px-2 py-0.5 text-[11px] font-semibold leading-5",
        toneClasses[tone]
      )}
    >
      {label}
    </span>
  );
}

export function BoundaryChip({
  label,
  tone = "neutral",
  title,
}: {
  label: string;
  tone?: ProductTone;
  title?: string;
}) {
  return <StatusChip label={label} tone={tone} title={title} />;
}

export function CapabilityCard({
  title,
  description,
  meta,
  tone = "neutral",
  children,
}: {
  title: string;
  description?: string;
  meta?: string;
  tone?: ProductTone;
  children?: ReactNode;
}) {
  return (
    <section className={cx("rounded-lg border p-3", toneClasses[tone])}>
      <div className="flex items-start justify-between gap-3">
        <div>
          <div className="text-sm font-semibold">{title}</div>
          {description && <p className="mt-1 text-xs leading-5 opacity-80">{description}</p>}
        </div>
        {meta && <span className="shrink-0 text-[11px] font-semibold opacity-70">{meta}</span>}
      </div>
      {children && <div className="mt-3">{children}</div>}
    </section>
  );
}

export function DangerZone({
  title,
  description,
  children,
}: {
  title: string;
  description: string;
  children: ReactNode;
}) {
  return (
    <section className="rounded-lg border border-rose-200 bg-rose-50 p-4 text-rose-900">
      <div className="text-sm font-semibold">{title}</div>
      <p className="mt-1 text-xs leading-5 text-rose-800">{description}</p>
      <div className="mt-3">{children}</div>
    </section>
  );
}

export function DecisionCard({
  title,
  eyebrow,
  description,
  tone = "neutral",
  children,
}: {
  title: string;
  eyebrow?: string;
  description?: string;
  tone?: ProductTone;
  children?: ReactNode;
}) {
  return (
    <section className={cx("rounded-lg border bg-white p-4", toneClasses[tone])}>
      {eyebrow && (
        <div className="text-[11px] font-semibold uppercase text-stone-500">{eyebrow}</div>
      )}
      <div className="mt-1 text-sm font-semibold text-stone-950">{title}</div>
      {description && <p className="mt-1 text-sm leading-6 text-stone-700">{description}</p>}
      {children && <div className="mt-4">{children}</div>}
    </section>
  );
}

export function TrustDrawer({
  title,
  subtitle,
  children,
  defaultOpen = false,
}: {
  title: string;
  subtitle?: string;
  children: ReactNode;
  defaultOpen?: boolean;
}) {
  return (
    <details
      open={defaultOpen}
      className="rounded-lg border border-stone-200 bg-white px-4 py-3 text-sm text-stone-700"
    >
      <summary className="cursor-pointer list-none">
        <div className="flex items-start justify-between gap-3">
          <div>
            <div className="font-semibold text-stone-950">{title}</div>
            {subtitle && <div className="mt-0.5 text-xs leading-5 text-stone-500">{subtitle}</div>}
          </div>
          <span className="rounded-md border border-stone-200 bg-stone-50 px-2 py-0.5 text-[11px] font-semibold text-stone-500">
            展开
          </span>
        </div>
      </summary>
      <div className="mt-3 border-t border-stone-100 pt-3">{children}</div>
    </details>
  );
}

export function EmptyState({
  title,
  description,
  action,
}: {
  title: string;
  description?: string;
  action?: ReactNode;
}) {
  return (
    <div className="rounded-lg border border-dashed border-stone-200 bg-white px-4 py-8 text-center">
      <div className="text-sm font-semibold text-stone-800">{title}</div>
      {description && <p className="mt-1 text-xs leading-5 text-stone-500">{description}</p>}
      {action && <div className="mt-4">{action}</div>}
    </div>
  );
}

export function TechnicalDetails({
  summary = "技术详情",
  children,
}: {
  summary?: string;
  children: ReactNode;
}) {
  return (
    <details className="rounded-lg border border-stone-200 bg-stone-50 px-3 py-2 text-xs text-stone-600">
      <summary className="cursor-pointer font-semibold text-stone-800">{summary}</summary>
      <div className="mt-2 space-y-1 leading-5">{children}</div>
    </details>
  );
}
