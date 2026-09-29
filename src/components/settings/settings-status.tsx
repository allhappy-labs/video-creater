import type { ReactNode } from "react";

export interface SettingsStatusProps {
  ariaLabel: string;
  label: string;
  value: string;
  detail?: string;
  icon?: ReactNode;
  tone?: "neutral" | "ready" | "warning" | "error";
}

const toneClasses = {
  neutral: "text-foreground",
  ready: "text-emerald-700",
  warning: "text-amber-700",
  error: "text-red-700",
} as const;

export function SettingsStatus({
  ariaLabel,
  label,
  value,
  detail,
  icon,
  tone = "neutral",
}: SettingsStatusProps) {
  return (
    <section
      role="status"
      aria-label={ariaLabel}
      className="flex items-start justify-between gap-3 border-t pt-3 text-xs"
    >
      <div className="min-w-0">
        <h2 className="font-semibold text-foreground">{label}</h2>
        {detail ? <p className="text-muted-foreground">{detail}</p> : null}
      </div>
      <span
        className={`flex shrink-0 items-center gap-1.5 font-medium ${toneClasses[tone]}`}
      >
        {icon}
        {value}
      </span>
    </section>
  );
}
