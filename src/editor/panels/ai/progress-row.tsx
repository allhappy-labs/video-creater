import { Loader2 } from "lucide-react";

interface ProgressRowProps {
  /** The one current phase, e.g. "Reviewing the timeline". */
  readonly label: string;
  /** Raw diagnostics text; the Details disclosure only appears when there is some. */
  readonly details?: string | null;
}

/** The current turn's phase as text, with an optional Details disclosure. */
export function ProgressRow({ label, details = null }: ProgressRowProps) {
  return (
    <div className="flex flex-col gap-1">
      <p role="status" className="flex items-center gap-2 text-[12.5px] text-muted-foreground">
        <Loader2 className="h-3.5 w-3.5 shrink-0 text-primary motion-safe:animate-spin" aria-hidden />
        {label}
      </p>
      {details?.trim() && (
        <details className="text-[12px] text-muted-foreground">
          <summary className="w-fit cursor-default rounded-md px-1 text-dim hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
            Details
          </summary>
          <pre className="mt-1 max-h-40 overflow-auto whitespace-pre-wrap break-words rounded-control bg-raised p-2 font-mono text-[11px]">{details}</pre>
        </details>
      )}
    </div>
  );
}
