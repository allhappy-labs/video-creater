import { useId, useState } from "react";
import { Dialog, DialogClose, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { pluralize } from "@/lib/format";
import { cn } from "@/lib/utils";
import { CaptionBuildFields } from "./caption-build-fields";
import type { CaptionBuildSettings } from "./captions-model";

interface RegenerateCaptionsDialogProps {
  readonly open: boolean;
  readonly captionCount: number;
  readonly settings: CaptionBuildSettings;
  onSettingsChange(settings: CaptionBuildSettings): void;
  onOpenChange(open: boolean): void;
  /** Resolves true when the captions were rebuilt. */
  onConfirm(): Promise<boolean>;
  readonly error: string | null;
}

const buttonClass =
  "h-8 rounded-control px-3 text-[13px] font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-panel disabled:pointer-events-none disabled:opacity-40";

/** Confirms replacing a transcript's existing captions with a fresh build. */
export function RegenerateCaptionsDialog({ open, captionCount, settings, onSettingsChange, onOpenChange, onConfirm, error }: RegenerateCaptionsDialogProps) {
  const descriptionId = useId();
  const [busy, setBusy] = useState(false);

  const confirm = async () => {
    if (busy) return;
    setBusy(true);
    const rebuilt = await onConfirm();
    setBusy(false);
    if (rebuilt) onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        aria-describedby={descriptionId}
        className="left-1/2 top-1/2 flex w-[min(360px,calc(100vw-32px))] -translate-x-1/2 -translate-y-1/2 flex-col gap-3 rounded-panel border border-line p-4"
      >
        <DialogTitle className="text-[15px] font-semibold">Regenerate {pluralize(captionCount, "caption")}?</DialogTitle>
        <p id={descriptionId} className="text-[13px] text-muted-foreground">
          Captions are rebuilt from the transcript. Text corrections, word emphasis and style changes on these captions are replaced. You can undo
          this.
        </p>
        <CaptionBuildFields settings={settings} disabled={busy} onChange={onSettingsChange} />
        {error && (
          <p role="alert" className="text-[12px] text-destructive">
            {error}
          </p>
        )}
        <div className="flex justify-end gap-2">
          <DialogClose className={cn(buttonClass, "text-foreground hover:bg-raised")}>Cancel</DialogClose>
          <button
            type="button"
            disabled={busy}
            onClick={() => void confirm()}
            className={cn(buttonClass, "bg-destructive text-destructive-foreground hover:bg-destructive/90")}
          >
            Regenerate
          </button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
