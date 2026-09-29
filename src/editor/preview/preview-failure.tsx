import { AlertCircle } from "lucide-react";
import { cn } from "@/lib/utils";

interface PreviewFailureProps {
  readonly title: string;
  readonly message: string;
  /** Shown under "Repair details". */
  readonly details: readonly string[];
  /** `failure` for failed loads or preparation, `notice` for pending or unavailable media. */
  readonly tone: "failure" | "notice";
  readonly onRetry?: (() => void) | undefined;
  readonly onOpenSource?: (() => void) | undefined;
}

const actionClassName =
  "h-7 rounded-md px-2.5 text-[12px] font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring";

/** The in-canvas preview issue message with optional Retry and Open source actions. */
export function PreviewFailure({ title, message, details, tone, onRetry, onOpenSource }: PreviewFailureProps) {
  return (
    <div
      role="alert"
      aria-label="Preview issue"
      className="absolute inset-x-2 top-2 z-30 flex max-h-[calc(100%-1rem)] items-start gap-2 overflow-auto rounded-control border border-line bg-popover/95 px-3 py-2 text-[12px] text-popover-foreground shadow-lg"
    >
      <AlertCircle className={cn("mt-0.5 h-4 w-4 shrink-0", tone === "failure" ? "text-destructive" : "text-warning")} aria-hidden />
      <div className="min-w-0 flex-1">
        <p className="font-medium">{title}</p>
        <p className="text-muted-foreground">{message}</p>
        {details.length > 0 && (
          <details className="mt-1 text-muted-foreground">
            <summary className="w-fit cursor-pointer select-none font-medium hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
              Repair details
            </summary>
            <ul className="mt-1 list-disc space-y-0.5 pl-4">
              {details.map((detail) => (
                <li key={detail}>{detail}</li>
              ))}
            </ul>
          </details>
        )}
        {(onRetry || onOpenSource) && (
          <div className="mt-2 flex flex-wrap gap-1.5">
            {onRetry && (
              <button type="button" onClick={onRetry} className={cn(actionClassName, "bg-accent text-accent-foreground hover:bg-accent/90")}>
                Retry preview
              </button>
            )}
            {onOpenSource && (
              <button type="button" onClick={onOpenSource} className={cn(actionClassName, "bg-raised text-foreground hover:bg-hover")}>
                Open source
              </button>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
