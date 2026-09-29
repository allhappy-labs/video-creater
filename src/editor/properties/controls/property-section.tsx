import { RotateCcw } from "lucide-react";
import { useId, type ReactNode } from "react";
import { IconButton } from "@/components/ui/icon-button";

interface PropertySectionProps {
  readonly title: string;
  /** Restores the section's defaults; omit for sections without a reset. */
  readonly onReset?: () => void;
  readonly resetDisabled?: boolean;
  /** Extra heading control, such as "Edit on canvas". */
  readonly action?: ReactNode;
  readonly children?: ReactNode;
}

/** A titled Properties section with an optional reset button and heading action. */
export function PropertySection({ title, onReset, resetDisabled = false, action, children }: PropertySectionProps) {
  const titleId = useId();
  return (
    <section aria-labelledby={titleId} className="flex flex-col gap-2.5 border-b border-line px-3 py-3 last:border-b-0">
      <div className="flex min-h-[26px] items-center justify-between gap-2">
        <h3 id={titleId} className="truncate text-[13px] font-semibold text-foreground">
          {title}
        </h3>
        <div className="flex shrink-0 items-center gap-1">
          {action}
          {onReset && (
            <IconButton label={`Reset ${title}`} size="sm" disabled={resetDisabled} onClick={onReset}>
              <RotateCcw className="h-3.5 w-3.5" aria-hidden />
            </IconButton>
          )}
        </div>
      </div>
      {children}
    </section>
  );
}
