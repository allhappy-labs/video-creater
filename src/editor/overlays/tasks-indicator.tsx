import { CircleCheck, LoaderCircle } from "lucide-react";
import { forwardRef, useEffect, useId, useMemo, useRef, useState, type ButtonHTMLAttributes } from "react";
import { indicatorState } from "@/lib/jobs/task-indicator";
import { cn } from "@/lib/utils";
import { useEditorStore } from "../store/editor-store-context";
import { TaskDetailsDialog } from "./task-details-dialog";
import { TasksPopover } from "./tasks-popover";

/** Re-evaluates the 10-minute window and the rows' "min ago" copy. */
const clockTickMs = 30_000;

function useNow(): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), clockTickMs);
    return () => clearInterval(timer);
  }, []);
  return now;
}

/**
 * The top-bar Background tasks pill: a spinner and the most important task, or a failed dot when the
 * newest task failed. Hidden when idle and 10 minutes after the last task finished. Also hosts the
 * task details dialog, so it stays reachable after the pill hides.
 */
export function TasksIndicator({ compact }: { readonly compact: boolean }) {
  const tasks = useEditorStore((state) => state.tasks);
  const now = useNow();
  // Tasks changing re-reads the clock, so a task that just finished isn't judged against a stale tick.
  const state = useMemo(() => indicatorState(tasks, Math.max(now, Date.now())), [tasks, now]);
  const [open, setOpen] = useState(false);
  const closeSheet = useEditorStore((store) => store.closeSheet);
  const buttonRef = useRef<HTMLButtonElement>(null);

  // Phones show one sheet at a time: the tasks sheet replaces a tab or tool sheet.
  function openCompactSheet() {
    closeSheet();
    setOpen(true);
  }

  return (
    <>
      {state.visible && (
        <TasksPopover compact={compact} open={open} onOpenChange={setOpen} now={now}>
          <IndicatorButton
            ref={buttonRef}
            compact={compact}
            tone={state.tone}
            label={state.label}
            progress={state.record.progress}
            newestFailed={state.newestFailed}
            {...(compact ? { onClick: openCompactSheet } : {})}
          />
        </TasksPopover>
      )}
      <TaskDetailsDialog returnFocusRef={buttonRef} />
    </>
  );
}

interface IndicatorButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  readonly compact: boolean;
  readonly tone: "running" | "failed" | "completed";
  readonly label: string;
  readonly progress: number | null;
  readonly newestFailed: boolean;
}

const IndicatorButton = forwardRef<HTMLButtonElement, IndicatorButtonProps>(function IndicatorButton(
  { compact, tone, label, progress, newestFailed, className, ...props },
  ref,
) {
  const labelId = useId();
  const percent = progress === null ? null : `${Math.round(Math.min(1, Math.max(0, progress)) * 100)}%`;
  const failedDot = (
    <span data-testid="tasks-failed-dot" className="h-2 w-2 shrink-0 rounded-full bg-destructive" aria-hidden />
  );
  // Colour is never the only signal: the spinner, dot and check differ in shape, and the label names the state.
  const description = tone === "running" && newestFailed ? `${label} · A task failed` : label;

  return (
    <button
      ref={ref}
      type="button"
      aria-label="Background tasks"
      aria-describedby={labelId}
      className={cn(
        "flex h-8 shrink-0 items-center gap-1.5 rounded-full bg-raised text-xs text-muted-foreground hover:bg-hover hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        compact ? "px-2.5" : "max-w-[220px] px-3",
        className,
      )}
      {...props}
    >
      {tone === "running" && (
        <LoaderCircle data-testid="tasks-spinner" className="h-3.5 w-3.5 shrink-0 text-primary motion-safe:animate-spin" aria-hidden />
      )}
      {tone === "failed" && failedDot}
      {tone === "completed" && <CircleCheck className="h-3.5 w-3.5 shrink-0 text-success" aria-hidden />}
      {compact ? (
        <>
          {tone === "running" && percent && <span className="tabular-time" aria-hidden>{percent}</span>}
          {tone === "failed" && <span aria-hidden>Failed</span>}
        </>
      ) : (
        <span className="truncate" aria-hidden>
          {label}
        </span>
      )}
      {tone === "running" && newestFailed && failedDot}
      <span id={labelId} className="sr-only">
        {description}
      </span>
    </button>
  );
});
