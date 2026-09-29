import { CircleAlert, CircleCheck } from "lucide-react";
import type { ReactNode } from "react";

export type LoadState<T> = { readonly status: "loading" } | { readonly status: "loaded"; readonly value: T } | { readonly status: "failed"; readonly message: string };

export function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function DetailSection({ title, children, action }: { readonly title: string; readonly children: ReactNode; readonly action?: ReactNode }) {
  return (
    <section aria-label={title} className="flex flex-col gap-1.5">
      <div className="flex min-h-7 items-center justify-between gap-2">
        <h3 className="text-xs font-semibold text-muted-foreground">{title}</h3>
        {action}
      </div>
      <dl className="grid grid-cols-[minmax(120px,max-content)_minmax(0,1fr)] gap-x-4 gap-y-1 text-[13px]">{children}</dl>
    </section>
  );
}

export function DetailRow({ label, mono = false, children }: { readonly label: string; readonly mono?: boolean; readonly children: ReactNode }) {
  return (
    <>
      <dt className="text-muted-foreground">{label}</dt>
      <dd className={mono ? "break-all font-mono text-xs leading-5" : "min-w-0 break-words"}>{children}</dd>
    </>
  );
}

export function StatusBadge({ ok, children }: { readonly ok: boolean; readonly children: ReactNode }) {
  const Icon = ok ? CircleCheck : CircleAlert;
  return (
    <span className={ok ? "inline-flex items-center gap-1 text-success" : "inline-flex items-center gap-1 text-warning"}>
      <Icon className="h-3.5 w-3.5 shrink-0" aria-hidden />
      {children}
    </span>
  );
}

