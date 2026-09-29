import { AlertTriangle, Check, Undo2, X, type LucideIcon } from "lucide-react";
import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import type { ResultFact } from "@/lib/agent/result-facts";
import { cn } from "@/lib/utils";

/** Shared chrome for the AI result, review and failure cards. */

type CardTone = "success" | "warning" | "danger" | "neutral";

const toneIcon: Record<CardTone, LucideIcon> = { success: Check, warning: AlertTriangle, danger: X, neutral: Undo2 };
const toneClass: Record<CardTone, string> = {
  success: "bg-success/15 text-success",
  warning: "bg-warning/15 text-warning",
  danger: "bg-destructive/15 text-destructive",
  neutral: "bg-hover text-muted-foreground",
};

interface ResultCardProps {
  /** The status text; it names the card and never relies on the icon's color. */
  readonly status: string;
  readonly tone: CardTone;
  readonly children?: ReactNode;
}

export const ResultCard = forwardRef<HTMLElement, ResultCardProps>(function ResultCard({ status, tone, children }, ref) {
  const Icon = toneIcon[tone];
  return (
    <article ref={ref} aria-label={status} className="mt-2 overflow-hidden rounded-[10px] bg-raised">
      <header className="flex items-center gap-2 px-3 py-2.5">
        <span aria-hidden className={cn("grid h-[18px] w-[18px] shrink-0 place-items-center rounded-full", toneClass[tone])}>
          <Icon className="h-[11px] w-[11px]" strokeWidth={3} />
        </span>
        <p className="min-w-0 break-words text-[13px] font-semibold text-foreground">{status}</p>
      </header>
      {children}
    </article>
  );
});

export function FactChips({ facts }: { readonly facts: readonly ResultFact[] }) {
  if (facts.length === 0) return null;
  return (
    <ul aria-label="Facts" className="flex flex-wrap gap-1.5 px-3 pb-2.5">
      {facts.map((fact, index) => (
        <li key={`${fact.kind}-${index.toString()}`} className="rounded-[10px] bg-panel px-2 py-[3px] text-[11.5px] text-muted-foreground">
          {fact.label}
        </li>
      ))}
    </ul>
  );
}

export function CardActions({ children }: { readonly children: ReactNode }) {
  return <div className="flex flex-wrap items-center gap-1.5 px-3 pb-3 pt-1">{children}</div>;
}

type CardButtonVariant = "primary" | "secondary" | "ghost";

const variantClass: Record<CardButtonVariant, string> = {
  primary: "bg-primary px-3 text-primary-foreground hover:bg-primary/90 aria-disabled:hover:bg-primary",
  secondary: "bg-panel px-3 text-foreground hover:bg-hover aria-disabled:hover:bg-panel",
  ghost: "px-2.5 text-muted-foreground hover:bg-hover hover:text-foreground aria-disabled:hover:bg-transparent",
};

interface CardButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  readonly variant?: CardButtonVariant;
}

/** Card buttons stay focusable when unavailable (`aria-disabled`), so their reason stays reachable. */
export const CardButton = forwardRef<HTMLButtonElement, CardButtonProps>(function CardButton({ variant = "secondary", className, ...props }, ref) {
  return (
    <button
      ref={ref}
      type="button"
      className={cn(
        "flex h-8 items-center gap-1.5 rounded-control text-[12.5px] font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring aria-disabled:cursor-not-allowed aria-disabled:opacity-50 motion-reduce:transition-none",
        variantClass[variant],
        className,
      )}
      {...props}
    />
  );
});
