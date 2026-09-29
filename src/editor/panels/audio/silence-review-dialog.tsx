import { useId, useState } from "react";
import { Dialog, DialogClose, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { silenceSummary } from "@/lib/audio/cleanup-summary";
import { formatTimecode, pluralize } from "@/lib/format";
import type { ProjectActionRippleDeleteRange } from "@/lib/project";
import { cn } from "@/lib/utils";
import { useEditorStoreApi } from "../../store/editor-store-context";

interface SilenceReviewDialogProps {
  readonly open: boolean;
  readonly ranges: readonly ProjectActionRippleDeleteRange[];
  onOpenChange(open: boolean): void;
  /** Resolves true when the cuts were applied; the dialog then closes. */
  onApply(ranges: readonly ProjectActionRippleDeleteRange[]): Promise<boolean>;
}

const buttonClass =
  "h-8 rounded-control px-3 text-[13px] font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-panel disabled:pointer-events-none disabled:opacity-40";

function rangeKey(range: ProjectActionRippleDeleteRange): string {
  return `${range.startSeconds.toString()}:${range.endSeconds.toString()}`;
}

/** "00:00:01.120" trimmed to "0:01.12" for compact range rows. */
function formatRangeTime(seconds: number): string {
  return formatTimecode(seconds)
    .replace(/^00:/, "")
    .replace(/^0(\d:)/, "$1")
    .replace(/(\.\d*?)0+$/, "$1")
    .replace(/\.$/, "");
}

function formatSaved(seconds: number): string {
  return `${seconds.toString()}s`;
}

interface SilenceReviewBodyProps extends Pick<SilenceReviewDialogProps, "ranges" | "onApply"> {
  readonly descriptionId: string;
}

/** The dialog body mounts on open, so every review starts with all ranges checked. */
function SilenceReviewBody({ ranges, onApply, descriptionId }: SilenceReviewBodyProps) {
  const store = useEditorStoreApi();
  const [excluded, setExcluded] = useState<ReadonlySet<string>>(() => new Set());
  const [busy, setBusy] = useState(false);
  const checked = ranges.filter((range) => !excluded.has(rangeKey(range)));
  const { secondsSaved } = silenceSummary(checked);

  const toggle = (key: string) =>
    setExcluded((current) => {
      const next = new Set(current);
      if (!next.delete(key)) next.add(key);
      return next;
    });

  const preview = (range: ProjectActionRippleDeleteRange) => {
    const state = store.getState();
    if (state.previewSource.kind !== "timeline") state.previewTimeline();
    state.seek(range.startSeconds);
  };

  const apply = async () => {
    if (checked.length === 0 || busy) return;
    setBusy(true);
    await onApply(checked);
    setBusy(false);
  };

  return (
    <>
      <DialogTitle className="text-[15px] font-semibold">Review pauses</DialogTitle>
      <p id={descriptionId} className="mt-1 text-[13px] text-muted-foreground">
        Checked pauses are cut and later clips move up. Select a pause to preview it. You can undo this.
      </p>
      <ul aria-label="Pauses" className="mt-3 flex max-h-[min(320px,50vh)] flex-col gap-1 overflow-y-auto">
        {ranges.map((range, index) => {
          const key = rangeKey(range);
          const number = (index + 1).toString();
          const length = Math.round((range.endSeconds - range.startSeconds) * 10) / 10;
          return (
            <li key={key} className="flex items-center gap-2 rounded-md bg-raised px-2">
              <input
                type="checkbox"
                aria-label={`Include pause ${number}`}
                checked={!excluded.has(key)}
                onChange={() => toggle(key)}
                className="h-3.5 w-3.5 shrink-0 accent-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              />
              <button
                type="button"
                aria-label={`Preview pause ${number} at ${formatRangeTime(range.startSeconds)}`}
                onClick={() => preview(range)}
                className="flex h-8 min-w-0 flex-1 items-center justify-between gap-2 rounded-md px-1 text-left text-[12px] text-foreground hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                <span className="truncate tabular-nums">
                  {formatRangeTime(range.startSeconds)} – {formatRangeTime(range.endSeconds)}
                </span>
                <span className="shrink-0 tabular-nums text-dim">{formatSaved(length)}</span>
              </button>
            </li>
          );
        })}
      </ul>
      <p className="mt-3 text-[12px] text-muted-foreground" aria-live="polite">
        {pluralize(checked.length, "pause")} selected · saves {formatSaved(secondsSaved)}
      </p>
      <div className="mt-4 flex justify-end gap-2">
        <DialogClose className={cn(buttonClass, "text-foreground hover:bg-raised")}>Cancel</DialogClose>
        <button
          type="button"
          disabled={checked.length === 0 || busy}
          onClick={() => void apply()}
          className={cn(buttonClass, "bg-primary text-primary-foreground hover:bg-primary/90")}
        >
          Apply {pluralize(checked.length, "cut")}
        </button>
      </div>
    </>
  );
}

/** Lists detected pauses with checkboxes; only the checked ranges are ripple-deleted, as one undo step. */
export function SilenceReviewDialog({ open, ranges, onOpenChange, onApply }: SilenceReviewDialogProps) {
  const descriptionId = useId();
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        aria-describedby={descriptionId}
        className="left-1/2 top-1/2 w-[min(400px,calc(100vw-32px))] -translate-x-1/2 -translate-y-1/2 rounded-panel border border-line p-4"
      >
        <SilenceReviewBody ranges={ranges} onApply={onApply} descriptionId={descriptionId} />
      </DialogContent>
    </Dialog>
  );
}
