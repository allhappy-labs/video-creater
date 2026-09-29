import type { ReactElement } from "react";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import type { TaskRecord } from "@/lib/jobs/task-records";
import { useRevealService } from "../services/reveal-service";
import { BottomSheet } from "../shell/bottom-sheet";
import { useEditorStore } from "../store/editor-store-context";
import { TaskRow } from "./task-row";

const title = "Background tasks";

interface TasksPopoverProps {
  /** Mobile: the list opens as a compact bottom sheet instead of a popover. */
  readonly compact: boolean;
  readonly open: boolean;
  onOpenChange(open: boolean): void;
  readonly now: number;
  /** The indicator button. In compact mode it must open the sheet itself. */
  readonly children: ReactElement;
}

/** The background task list, the editor's only job surface: a popover under the indicator, or a bottom sheet on phones. */
export function TasksPopover({ compact, open, onOpenChange, now, children }: TasksPopoverProps) {
  if (compact) {
    return (
      <>
        {children}
        <BottomSheet title={title} open={open} height="compact" onClose={() => onOpenChange(false)}>
          <TaskList now={now} onDetailsOpened={() => onOpenChange(false)} className="px-3.5 pb-3" />
        </BottomSheet>
      </>
    );
  }
  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverTrigger asChild>{children}</PopoverTrigger>
      <PopoverContent
        aria-label={title}
        // Focus the list itself: a first-row action taking focus would pop its tooltip over the list.
        onOpenAutoFocus={(event) => {
          event.preventDefault();
          if (event.currentTarget instanceof HTMLElement) event.currentTarget.focus();
        }}
        className="flex max-h-[min(480px,var(--radix-popover-content-available-height))] w-[360px] max-w-[calc(100vw-16px)] flex-col px-3 py-2">
        <p className="shrink-0 px-1 pb-1 pt-1.5 font-semibold">{title}</p>
        <TaskList now={now} onDetailsOpened={() => onOpenChange(false)} className="min-h-0 overflow-y-auto" />
      </PopoverContent>
    </Popover>
  );
}

function TaskList({ now, onDetailsOpened, className }: { readonly now: number; onDetailsOpened(): void; readonly className: string }) {
  const tasks: readonly TaskRecord[] = useEditorStore((state) => state.tasks);
  const cancelTask = useEditorStore((state) => state.cancelTask);
  const retryTask = useEditorStore((state) => state.retryTask);
  const openTaskDetails = useEditorStore((state) => state.openTaskDetails);
  const reveal = useRevealService();

  if (tasks.length === 0) return <p className="px-1 py-3 text-xs text-muted-foreground">No background tasks.</p>;
  return (
    <ul aria-label={title} className={className}>
      {tasks.map((record) => (
        <TaskRow
          key={record.id}
          record={record}
          now={now}
          onReveal={(path) => void reveal.revealExportArtifact(path)}
          onCancel={(id) => void cancelTask(id)}
          onRetry={(id) => void retryTask(id)}
          onDetails={(id) => {
            // Close the list first so the details dialog takes focus and returns it to the indicator.
            onDetailsOpened();
            openTaskDetails(id);
          }}
        />
      ))}
    </ul>
  );
}
