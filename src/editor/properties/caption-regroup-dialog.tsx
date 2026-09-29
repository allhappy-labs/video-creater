import { useId, useState } from "react";
import { Dialog, DialogClose, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { cn } from "@/lib/utils";
import { useEditorStore, useEditorStoreApi } from "../store/editor-store-context";
import { captionRegroup } from "./caption-edits";

interface CaptionRegroupDialogProps {
  readonly itemId: string;
  /** The requested max words per line; null keeps the dialog closed. */
  readonly wordsPerCue: number | null;
  onClose(): void;
}

const buttonClass =
  "h-8 rounded-control px-3 text-[13px] font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-panel disabled:pointer-events-none disabled:opacity-40";

/**
 * Confirms the destructive max-words regroup, naming the cue count change. Confirming applies the
 * regroup as one batch, selects the rebuilt cue under the old selection, and shows failures inline.
 */
export function CaptionRegroupDialog({ itemId, wordsPerCue, onClose }: CaptionRegroupDialogProps) {
  const store = useEditorStoreApi();
  const project = useEditorStore((state) => state.project);
  const descriptionId = useId();
  const [failure, setFailure] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const plan = wordsPerCue === null ? null : captionRegroup(project, itemId, wordsPerCue);
  const blocked = plan !== null && "blocked" in plan ? plan.blocked : null;
  const ready = plan !== null && !("blocked" in plan) ? plan : null;
  const message = failure ?? blocked;

  const close = () => {
    setFailure(null);
    onClose();
  };

  const confirm = async () => {
    if (!ready || busy) return;
    setBusy(true);
    const state = store.getState();
    state.clearPropertyPreview();
    const result = await state.applyActions(ready.actions);
    setBusy(false);
    if (result === null) {
      setFailure(store.getState().lastError ?? "The captions could not be regrouped.");
      return;
    }
    if (ready.selectItemId) store.getState().selectItems([ready.selectItemId]);
    close();
  };

  const cueCount = (count: number) => `${count.toString()} ${count === 1 ? "cue" : "cues"}`;
  const title = ready ? `Regroup ${cueCount(ready.fromCount)} into ${ready.toCount.toString()}?` : "Can't regroup captions";

  return (
    <Dialog
      open={wordsPerCue !== null}
      onOpenChange={(open) => {
        if (!open) close();
      }}
    >
      <DialogContent
        aria-describedby={descriptionId}
        className="left-1/2 top-1/2 w-[min(360px,calc(100vw-32px))] -translate-x-1/2 -translate-y-1/2 rounded-panel border border-line p-4"
      >
        <DialogTitle className="text-[15px] font-semibold">{title}</DialogTitle>
        <p id={descriptionId} className="mt-1 text-[13px] text-muted-foreground">
          Captions are rebuilt from the transcript with up to {wordsPerCue?.toString() ?? ""} words per line. Text corrections and word
          emphasis in this caption group are replaced. You can undo this.
        </p>
        {message && (
          <p role="alert" className="mt-3 text-[12px] text-destructive">
            {message}
          </p>
        )}
        <div className="mt-4 flex justify-end gap-2">
          <DialogClose className={cn(buttonClass, "text-foreground hover:bg-raised")}>Cancel</DialogClose>
          <button
            type="button"
            disabled={!ready || busy}
            onClick={() => void confirm()}
            className={cn(buttonClass, "bg-destructive text-destructive-foreground hover:bg-destructive/90")}
          >
            Regroup
          </button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
